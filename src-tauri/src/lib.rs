//! Veritas core: Tauri setup and module wiring (doc-2 §1.6).

use tauri::{Emitter, Manager};

mod audio;
mod commands;
// Query functions are consumed from VRT-7/VRT-8 on; remove this allow there.
#[allow(dead_code)]
mod db;
// The Engine trait is consumed by live transcription (VRT-16); remove this allow there.
#[allow(dead_code)]
mod engine;
mod media_server;
mod paths;
mod speakers;
#[cfg(test)]
mod test_util;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Open the database and run migrations before any window can use it.
            let paths = paths::AppPaths::resolve(app.path().app_data_dir()?);
            std::fs::create_dir_all(paths.meetings_root())?;
            let db = tauri::async_runtime::block_on(db::Db::open(&paths.database()))?;
            let media = media_server::MediaServer::start(paths.clone())?;
            let handle = app.handle().clone();
            let engine = engine::EngineManager::new(paths.clone(), move |status| {
                let _ = handle.emit("engine:status", status);
            });
            app.manage(db);
            app.manage(media);
            app.manage(engine);
            app.manage(paths);
            Ok(())
        })
        .manage(commands::RecorderState::default())
        .manage(commands::MonitorState::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::list_sources,
            commands::start_recording,
            commands::pause_recording,
            commands::resume_recording,
            commands::stop_recording,
            commands::current_recording,
            commands::set_source_muted,
            commands::rename_meeting,
            commands::get_recording_sources,
            commands::set_recording_sources,
            commands::start_source_monitor,
            commands::stop_source_monitor,
            commands::list_meetings,
            commands::delete_meeting,
            commands::get_meeting,
            commands::engine_status,
            commands::engine_download,
            commands::engine_cancel_download,
            commands::engine_start,
            commands::engine_notice
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Stop the engine helper with the app (it also exits when its stdin closes).
            if let tauri::RunEvent::Exit = event
                && let Some(engine) = app.try_state::<engine::EngineManager>()
            {
                engine.shutdown();
            }
        });
}
