//! Spike (VRT-40): global shortcuts through the XDG GlobalShortcuts portal on GNOME Wayland.
//! Usage: shortcuts-spike [<app id>] [<seconds>]
use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> ashpd::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let app_id = args.get(1).cloned().unwrap_or_else(|| "ai.agenticcraft.manent".into());
    let secs: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(90);
    match ashpd::AppID::try_from(app_id.as_str()) {
        Ok(id) => match ashpd::register_host_app(id).await {
            Ok(()) => println!("registered host app {app_id}"),
            Err(e) => println!("register {app_id} failed: {e}"),
        },
        Err(e) => println!("invalid app id {app_id}: {e}"),
    }
    let portal = GlobalShortcuts::new().await?;
    println!("portal version {}", portal.version());
    let session = portal.create_session(Default::default()).await?;
    let wanted = [
        NewShortcut::new("toggle-recording", "Start or stop recording").preferred_trigger("CTRL+ALT+R"),
        NewShortcut::new("mark-moment", "Mark this moment").preferred_trigger("CTRL+ALT+M"),
    ];
    let bound = portal
        .bind_shortcuts(&session, &wanted, None, Default::default())
        .await?
        .response()?;
    for s in bound.shortcuts() {
        println!("bound {} -> '{}'", s.id(), s.trigger_description());
    }
    let listed = portal.list_shortcuts(&session, Default::default()).await?.response()?;
    println!("listed {} shortcuts", listed.shortcuts().len());
    let mut activated = portal.receive_activated().await?;
    let deadline = tokio::time::sleep(std::time::Duration::from_secs(secs));
    tokio::pin!(deadline);
    println!("press the shortcuts now ({secs} s)");
    loop {
        tokio::select! {
            Some(a) = activated.next() => println!("activated {} at {:?}", a.shortcut_id(), a.timestamp()),
            _ = &mut deadline => break,
        }
    }
    Ok(())
}
