//! Dictation's clipboard and notifications on macOS and Windows come with their global
//! shortcuts (decision-10); until then the dictation shortcut can't be bound there.

pub async fn copy(_text: &str) -> Result<(), String> {
    Err("Dictation is not available on this system yet.".into())
}

pub async fn notify(_id: &str, title: &str, body: &str) -> Result<(), String> {
    eprintln!("dictation: {title}: {body}");
    Ok(())
}

/// Auto-paste comes with the platform's dictation code.
pub const AUTOPASTE: bool = false;

pub struct AutoPaste;

impl AutoPaste {
    pub async fn start(_token: Option<&str>) -> Result<(Self, Option<String>), String> {
        Err("Pasting automatically is not available on this system yet.".into())
    }

    pub async fn paste(&self, _text: &str, _key: super::PasteKey) -> Result<super::Pasted, String> {
        Err("not available".into())
    }

    pub fn is_alive(&self) -> bool {
        false
    }

    pub async fn close(self) {}
}
