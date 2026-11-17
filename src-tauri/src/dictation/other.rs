//! Dictation's clipboard and notifications on macOS and Windows come with their global
//! shortcuts (decision-10); until then the dictation shortcut can't be bound there.

pub async fn copy(_text: &str) -> Result<(), String> {
    Err("Dictation is not available on this system yet.".into())
}

pub async fn notify(_id: &str, title: &str, body: &str) -> Result<(), String> {
    eprintln!("dictation: {title}: {body}");
    Ok(())
}
