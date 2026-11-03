//! Keeping the computer awake while a meeting is recorded (VRT-36, decision-6).
//!
//! Idle suspend during a recording freezes capture and loses that part of the meeting
//! (seen in VRT-35). While a [`SleepBlock`] lives, the OS doesn't go to sleep on its own;
//! the screen may still lock and blank as usual, and closing the lid still suspends (the
//! user's choice; the recorder then marks the missing audio).
//!
//! The block is held by a small OS tool run as a child process, so no D-Bus or system API
//! binding is needed, and the block ends with the app even if it crashes: the child reads
//! our stdin pipe (Linux) or watches our pid (macOS) and exits when we are gone.

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod platform;
#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod platform;
#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod platform;

use std::process::Child;

/// Prevents idle sleep until dropped.
pub struct SleepBlock {
    child: Option<Child>,
}

impl SleepBlock {
    /// Ask the OS not to sleep. Never fails the caller: without the tool (or on a platform
    /// not supported yet) it logs why and holds nothing.
    pub fn acquire(why: &str) -> Self {
        match platform::spawn(why) {
            Ok(child) => Self { child },
            Err(e) => {
                eprintln!("keeping the computer awake failed: {e}");
                Self { child: None }
            }
        }
    }

    /// Whether a block is actually held.
    pub fn is_held(&self) -> bool {
        self.child.is_some()
    }
}

impl Drop for SleepBlock {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    fn held_by_us() -> bool {
        let out = std::process::Command::new("systemd-inhibit")
            .arg("--list")
            .output()
            .expect("systemd-inhibit --list");
        let pid = std::process::id().to_string();
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .any(|l| l.contains("Manent") && l.contains(&format!("unit test {pid}")))
    }

    #[test]
    #[ignore = "needs a systemd session (logind); run with --ignored"]
    fn block_is_listed_while_held_and_gone_after_drop() {
        let block = SleepBlock::acquire(&format!("unit test {}", std::process::id()));
        assert!(block.is_held());
        let t = std::time::Instant::now();
        while !held_by_us() && t.elapsed().as_secs() < 3 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(held_by_us(), "inhibitor listed by logind");
        drop(block);
        let t = std::time::Instant::now();
        while held_by_us() && t.elapsed().as_secs() < 3 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(!held_by_us(), "inhibitor released");
    }
}
