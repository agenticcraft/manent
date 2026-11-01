//! macOS: `caffeinate -i -w <pid>` holds a "prevent idle system sleep" power assertion
//! until our process exits (or we kill it). Not yet verified on a Mac (VRT-12).

use std::process::{Child, Command, Stdio};

pub fn spawn(_why: &str) -> Result<Option<Child>, String> {
    Command::new("caffeinate")
        .args(["-i", "-w", &std::process::id().to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(Some)
        .map_err(|e| format!("caffeinate: {e}"))
}
