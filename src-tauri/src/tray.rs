//! Tray icon (VRT-22): shows whether a meeting is being recorded and starts or stops one,
//! also while the window is hidden. On Linux it is an AppIndicator (GNOME needs the
//! AppIndicator extension, which Fedora ships; without it there is simply no icon).
//!
//! Start and stop run the same commands as the buttons in the window; those commands update
//! the icon and send `recording:started` / `recording:stopped`, so the window follows.

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

use crate::commands::{self, RecorderState};

const ID: &str = "main";
const TOGGLE: &str = "toggle-recording";
const SHOW: &str = "show";
const QUIT: &str = "quit";
const IDLE_ICON: &[u8] = include_bytes!("../icons/tray.png");
const RECORDING_ICON: &[u8] = include_bytes!("../icons/tray-recording.png");

/// The toggle menu item, kept to change its text.
pub struct TrayToggle(MenuItem<tauri::Wry>);

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, TOGGLE, "Start recording", true, None::<&str>)?;
    let show = MenuItem::with_id(app, SHOW, "Show Manent", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, QUIT, "Quit Manent", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &show, &quit_item])?;
    TrayIconBuilder::with_id(ID)
        .icon(Image::from_bytes(IDLE_ICON)?)
        .tooltip("Manent")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TOGGLE => toggle_recording(app.clone()),
            SHOW => show_window(app),
            QUIT => quit(app.clone()),
            _ => {}
        })
        .build(app)?;
    app.manage(TrayToggle(toggle));
    Ok(())
}

/// Show whether a recording runs: icon, tooltip and the menu's toggle.
pub fn set_recording(app: &AppHandle, recording: bool) {
    if let Some(tray) = app.tray_by_id(ID) {
        let icon = if recording { RECORDING_ICON } else { IDLE_ICON };
        if let Ok(image) = Image::from_bytes(icon) {
            let _ = tray.set_icon(Some(image));
        }
        let _ = tray.set_tooltip(Some(if recording {
            "Manent: recording"
        } else {
            "Manent"
        }));
    }
    if let Some(toggle) = app.try_state::<TrayToggle>() {
        let _ = toggle.0.set_text(if recording {
            "Stop recording"
        } else {
            "Start recording"
        });
    }
}

pub fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

async fn is_recording(app: &AppHandle) -> bool {
    app.state::<RecorderState>().is_recording_now().await
}

/// Start a recording, or stop the one running.
pub fn toggle_recording(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let result = if is_recording(&app).await {
            stop(&app).await
        } else {
            commands::start_recording(
                app.clone(),
                app.state(),
                app.state(),
                app.state(),
                app.state(),
                app.state(),
            )
            .await
            .map(|_| ())
        };
        if let Err(e) = result {
            eprintln!("tray: {e}");
            show_window(&app);
        }
    });
}

async fn stop(app: &AppHandle) -> Result<(), String> {
    commands::stop_recording(app.clone(), app.state(), app.state(), app.state())
        .await
        .map(|_| ())
}

/// Quit, finishing a running recording first so its files are complete.
pub fn quit(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if is_recording(&app).await
            && let Err(e) = stop(&app).await
        {
            eprintln!("tray: stopping the recording before quitting failed: {e}");
        }
        app.exit(0);
    });
}
