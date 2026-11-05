//! Global shortcuts on macOS and Windows come with their audio code (VRT-11, VRT-12;
//! decision-10: tauri-plugin-global-shortcut there).

use super::{Action, ShortcutView};

pub const SUPPORTED: bool = false;

pub struct Running;

impl Running {
    pub async fn stop(self) {}
}

pub async fn bind(
    _on_action: impl Fn(Action) + Send + Sync + 'static,
    _on_changed: impl Fn(Vec<ShortcutView>) + Send + Sync + 'static,
) -> Result<(Running, Vec<ShortcutView>), String> {
    Err("Global shortcuts are not available on this system yet.".into())
}

pub fn open_settings() -> Result<(), String> {
    Err("Not available on this system yet.".into())
}
