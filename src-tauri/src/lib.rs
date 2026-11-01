//! Veritas core: Tauri setup and module wiring (doc-2 §1.6).

use tauri::{Emitter, Manager};

mod audio;
mod commands;
// Query functions are consumed from VRT-7/VRT-8 on; remove this allow there.
#[allow(dead_code)]
mod db;
mod engine;
mod export;
mod jobs;
mod live;
mod media_server;
mod passes;
mod paths;
mod power;
mod rolling;
mod speakers;
#[cfg(test)]
mod test_util;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // File dialog for "Import audio" (VRT-19, decision-3).
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Open the database and run migrations before any window can use it.
            let paths = paths::AppPaths::resolve(app.path().app_data_dir()?);
            std::fs::create_dir_all(&paths.data)?;
            let db = tauri::async_runtime::block_on(db::Db::open(&paths.database()))?;
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
            // A transcription interrupted by quitting the app runs again.
            for m in tauri::async_runtime::block_on(db.list_meetings())? {
                if m.status == "processing" {
                    let name = if m.title.is_empty() {
                        "Untitled meeting".to_string()
                    } else {
                        m.title
                    };
                    jobs.transcribe(m.id, name);
                }
            }
            app.manage(db);
            app.manage(media);
            app.manage(engine);
            app.manage(jobs);
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
            commands::live_transcript,
            commands::get_me_name,
            commands::set_me_name,
            commands::meeting_voices,
            commands::meeting_gaps,
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
