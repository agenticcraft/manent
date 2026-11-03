//! Linux: a logind inhibitor through `systemd-inhibit` (part of systemd).
//!
//! `--what=sleep --mode=block` makes logind refuse an idle suspend (GNOME, KDE and others
//! ask logind to suspend). The command it runs, `cat`, waits on our stdin pipe: when the
//! app exits or crashes the pipe closes, `cat` ends and so does the inhibitor.

use std::process::{Child, Command, Stdio};

pub fn spawn(why: &str) -> Result<Option<Child>, String> {
    Command::new("systemd-inhibit")
        .args([
            "--what=sleep",
            "--mode=block",
            "--who=Manent",
            &format!("--why={why}"),
            "cat",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(Some)
        .map_err(|e| format!("systemd-inhibit: {e}"))
}
