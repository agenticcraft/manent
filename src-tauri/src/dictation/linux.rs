//! Clipboard and notifications for dictation on Linux (decision-11): the clipboard through
//! `wl-copy` (wl-clipboard), which can set it while another window has focus; notifications
//! through the XDG Notification portal.

use std::io::Write;
use std::process::{Command, Stdio};

use ashpd::desktop::notification::{Notification, NotificationProxy, Priority};

/// Put `text` on the clipboard. wl-copy keeps serving it from the background after it exits.
pub async fn copy(text: &str) -> Result<(), String> {
    let text = text.to_string();
    tauri::async_runtime::spawn_blocking(move || copy_blocking("wl-copy", &text))
        .await
        .map_err(|e| e.to_string())?
}

fn copy_blocking(program: &str, text: &str) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(["--type", "text/plain;charset=utf-8"])
        .stdin(Stdio::piped())
        // The background process wl-copy leaves must not hold on to our pipes.
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => {
                "Copying needs wl-copy: install the wl-clipboard package.".to_string()
            }
            _ => format!("Could not start wl-copy: {e}"),
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| format!("wl-copy: {e}"))?;
    }
    let status = child.wait().map_err(|e| format!("wl-copy: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Copying to the clipboard failed (wl-copy {status}); dictation needs a Wayland session."
        ))
    }
}

pub async fn notify(id: &str, title: &str, body: &str) -> Result<(), String> {
    let proxy = NotificationProxy::new().await.map_err(|e| e.to_string())?;
    let notification = Notification::new(title)
        .body(body)
        .priority(Priority::Normal);
    proxy
        .add_notification(id, notification)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_wl_copy_it_says_what_to_install() {
        let err = copy_blocking("manent-no-such-wl-copy", "x").unwrap_err();
        assert!(err.contains("install the wl-clipboard package"), "{err}");
    }

    /// Needs a Wayland session with wl-clipboard; run it in the isolated test session:
    /// `spikes/dictation/run-isolated.sh python3 spikes/dictation/with_keyboard.py \
    /// cargo test --manifest-path src-tauri/Cargo.toml --lib -- --ignored wayland_clipboard`.
    #[test]
    #[ignore]
    fn wayland_clipboard_keeps_special_characters() {
        let text = "Café, één \"quote\" 'single' — ë ï\nTweede regel\n\tmet tab";
        copy_blocking("wl-copy", text).expect("copy");
        let out = Command::new("wl-paste")
            .arg("--no-newline")
            .output()
            .expect("wl-paste");
        assert_eq!(String::from_utf8_lossy(&out.stdout), text);
    }
}
