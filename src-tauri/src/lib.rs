//! Veritas core: Tauri setup and module wiring (doc-2 §1.6).

use tauri::Manager;

mod audio;
mod commands;
// Query functions are consumed from VRT-7/VRT-8 on; remove this allow there.
#[allow(dead_code)]
mod db;
mod engine;
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
            app.manage(db);
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
            commands::delete_meeting
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
