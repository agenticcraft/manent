//! Veritas core: Tauri setup and module wiring (doc-2 §1.6).

use tauri::Manager;

mod audio;
mod commands;
// Query functions are consumed from VRT-7/VRT-8 on; remove this allow there.
#[allow(dead_code)]
mod db;
mod engine;
mod speakers;
#[cfg(test)]
mod test_util;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Open the database and run migrations before any window can use it.
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = tauri::async_runtime::block_on(db::Db::open(&dir.join("veritas.db")))?;
            app.manage(db);
            Ok(())
        })
        .manage(commands::RecorderState::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::list_sources,
            commands::start_recording,
            commands::pause_recording,
            commands::resume_recording,
            commands::stop_recording
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
