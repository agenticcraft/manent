//! Manent core: Tauri setup and module wiring (doc-2 §1.6).

use tauri::{Emitter, Manager};

mod audio;
mod commands;
// Query functions are consumed from VRT-7/VRT-8 on; remove this allow there.
#[allow(dead_code)]
mod db;
mod engine;
mod export;
mod jobs;
mod legacy;
mod live;
mod media_server;
mod open_url;
mod passes;
mod paths;
mod power;
mod rolling;
mod shortcuts;
mod speakers;
mod sync;
#[cfg(test)]
mod test_util;
mod transcript_md;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // File dialog for "Import audio" (VRT-19, decision-3).
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Open the database and run migrations before any window can use it.
            // Data from before the rename to Manent moves along first (VRT-44).
            let app_data = app.path().app_data_dir()?;
            let moved = if legacy::env_var("DATA_DIR").is_none() {
                legacy::move_data_dir(&app_data)?
            } else {
                None
            };
            let paths = paths::AppPaths::resolve(app_data);
            std::fs::create_dir_all(&paths.data)?;
            legacy::rename_database(&paths.data, paths::DATABASE_FILE)?;
            let db = tauri::async_runtime::block_on(db::Db::open(&paths.database()))?;
            tauri::async_runtime::block_on(legacy::after_open(&db, moved.as_deref(), &paths.data))?;
            // A meetings folder chosen in Settings > Storage (VRT-27).
            let chosen: Option<Option<String>> =
                tauri::async_runtime::block_on(db.get_setting(paths::MEETINGS_DIR_SETTING))?;
            if let Some(dir) = chosen.flatten() {
                paths.set_meetings_root(Some(dir.into()));
            }
            std::fs::create_dir_all(paths.meetings_root())?;
            let media = media_server::MediaServer::start(paths.clone())?;
            let handle = app.handle().clone();
            let engine = engine::EngineManager::new(paths.clone(), move |status| {
                let _ = handle.emit("engine:status", status);
            });
            let jobs = jobs::Jobs::new(
                Some(app.handle().clone()),
                db.clone(),
                engine.clone(),
                paths.clone(),
            );
            // A transcription interrupted by quitting the app runs again; a recording cut off
            // because the app died is recovered and then transcribed (VRT-41).
            for m in tauri::async_runtime::block_on(db.list_meetings())? {
                let name = if m.title.is_empty() {
                    "Untitled meeting".to_string()
                } else {
                    m.title
                };
                match m.status.as_str() {
                    "processing" => {
                        jobs.transcribe(m.id, name);
                    }
                    "recording" => {
                        jobs.recover(m.id, name);
                    }
                    _ => {}
                }
            }
            // Meetings from before VRT-37 get their transcript.md.
            transcript_md::spawn_backfill(db.clone());
            // Nextcloud sync (VRT-39): idle until connected.
            app.manage(sync::Sync::start(
                Some(app.handle().clone()),
                db.clone(),
                jobs.clone(),
                paths.clone(),
            ));
            // Global shortcuts (VRT-40), if the user turned them on.
            shortcuts::start(app.handle(), &db);
            app.manage(db);
            // Tray icon (VRT-22); the app works without one (e.g. a desktop without a tray).
            if let Err(e) = tray::build(app.handle()) {
                eprintln!("tray icon: {e}");
            }
            app.manage(media);
            app.manage(engine);
            app.manage(jobs);
            app.manage(paths);
            Ok(())
        })
        // Closing the window during a recording hides it: the recording goes on, and the
        // tray brings the window back or stops it (VRT-22).
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let recording = window
                    .app_handle()
                    .try_state::<commands::RecorderState>()
                    .is_some_and(|s| s.is_recording());
                if recording && window.app_handle().tray_by_id("main").is_some() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .manage(commands::RecorderState::default())
        .manage(shortcuts::Shortcuts::default())
        .manage(commands::MonitorState::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::list_sources,
            commands::start_recording,
            commands::pause_recording,
            commands::resume_recording,
            commands::stop_recording,
            commands::current_recording,
            commands::set_recording_in_room,
            commands::get_in_room,
            commands::mark_moment,
            commands::list_marks,
            commands::set_mark_label,
            commands::delete_mark,
            shortcuts::get_shortcuts,
            shortcuts::set_shortcuts_enabled,
            shortcuts::open_shortcut_settings,
            commands::set_source_muted,
            commands::rename_meeting,
            commands::get_recording_sources,
            commands::set_recording_sources,
            commands::start_source_monitor,
            commands::stop_source_monitor,
            commands::list_meetings,
            commands::delete_meeting,
            commands::get_meeting,
            commands::live_transcript,
            commands::get_me_name,
            commands::set_me_name,
            commands::meeting_voices,
            commands::meeting_gaps,
            commands::sync_status,
            commands::sync_connect,
            commands::sync_cancel_connect,
            commands::sync_open_login,
            commands::sync_disconnect,
            commands::sync_set_mode,
            commands::sync_now,
            commands::set_utc_offset,
            commands::name_speaker,
            commands::confirm_suggestion,
            commands::merge_labels,
            commands::list_people,
            commands::rename_person,
            commands::merge_people,
            commands::delete_person,
            commands::meeting_waveform,
            commands::export_meeting,
            commands::search_transcripts,
            commands::import_audio,
            commands::retranscribe,
            commands::list_jobs,
            commands::dismiss_job,
            commands::get_app_settings,
            commands::set_rolling_interval,
            commands::set_keep_audio,
            commands::move_meetings,
            commands::engine_check,
            commands::engine_remove,
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
