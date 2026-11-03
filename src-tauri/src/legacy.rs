//! Carrying data over from the app's working name, Veritas (VRT-44, decision-8).
//!
//! The identifier sets the OS data folder, so renaming `nl.veritas.app` to
//! `ai.agenticcraft.manent` would leave everything behind. On the first start of Manent:
//! - the old data folder moves to the new place (a rename; a copy and delete across disks),
//! - `veritas.db` becomes `manent.db`,
//! - paths in the database that point into the old folder are rewritten,
//! - a Nextcloud connection keeps its "Veritas" server folder (sync::KEYCHAIN also falls back
//!   to the old keychain entry and moves it),
//! - `VERITAS_*` environment variables still work, with a warning.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use crate::db::Db;

/// The working name's identifier (its OS data folder).
pub const OLD_IDENTIFIER: &str = "nl.veritas.app";
pub const OLD_DATABASE: &str = "veritas.db";
/// The server folder a connection made before the rename uses.
pub const OLD_SYNC_FOLDER: &str = "Veritas";

/// `MANENT_<name>`, or the old `VERITAS_<name>` (with a warning).
pub fn env_var(name: &str) -> Option<OsString> {
    if let Some(v) = std::env::var_os(format!("MANENT_{name}")) {
        return Some(v);
    }
    let old = format!("VERITAS_{name}");
    let v = std::env::var_os(&old)?;
    eprintln!("{old} is the old name; use MANENT_{name}");
    Some(v)
}

/// Whether `dir` holds no Manent data yet. The webview may already have put its caches there
/// (WebKit creates them before the app's setup runs), so only the database and the meetings
/// count.
fn has_no_data(dir: &Path) -> bool {
    !dir.join(crate::paths::DATABASE_FILE).exists()
        && !dir.join(OLD_DATABASE).exists()
        && !dir.join("meetings").exists()
}

/// Move the old app data folder (a sibling of `new`) into `new` if Manent has no data yet.
/// Entries already in `new` (the webview's caches) stay; the rest moves. Returns the old path
/// when something was moved.
pub fn move_data_dir(new: &Path) -> io::Result<Option<PathBuf>> {
    let Some(parent) = new.parent() else {
        return Ok(None);
    };
    let old = parent.join(OLD_IDENTIFIER);
    if !old.is_dir() || !has_no_data(new) {
        return Ok(None);
    }
    std::fs::create_dir_all(new)?;
    for entry in std::fs::read_dir(&old)? {
        let entry = entry?;
        let dest = new.join(entry.file_name());
        if dest.exists() {
            continue;
        }
        if std::fs::rename(entry.path(), &dest).is_err() {
            // Another disk: copy, the original goes with the old folder below.
            if entry.file_type()?.is_dir() {
                crate::paths::copy_dir(&entry.path(), &dest)?;
            } else {
                std::fs::copy(entry.path(), &dest)?;
            }
        }
    }
    std::fs::remove_dir_all(&old)?;
    eprintln!(
        "moved the Veritas data from {} to {}",
        old.display(),
        new.display()
    );
    Ok(Some(old))
}

/// `veritas.db` (and its WAL files) becomes `manent.db`, unless that exists already.
pub fn rename_database(data: &Path, new_name: &str) -> io::Result<()> {
    let (old, new) = (data.join(OLD_DATABASE), data.join(new_name));
    if !old.is_file() || new.exists() {
        return Ok(());
    }
    std::fs::rename(&old, &new)?;
    for suffix in ["-wal", "-shm"] {
        let side = data.join(format!("{OLD_DATABASE}{suffix}"));
        if side.is_file() {
            std::fs::rename(&side, data.join(format!("{new_name}{suffix}")))?;
        }
    }
    Ok(())
}

/// After the database opened: point paths into the old data folder at the new one, and keep a
/// sync connection's server folder.
pub async fn after_open(db: &Db, moved_from: Option<&Path>, data: &Path) -> Result<(), String> {
    if let Some(old) = moved_from {
        let n = db
            .rewrite_path_prefix(&old.display().to_string(), &data.display().to_string())
            .await
            .map_err(|e| e.to_string())?;
        eprintln!("rewrote {n} paths to the new data folder");
    }
    let server: Option<String> = db
        .get_setting(crate::sync::SERVER_SETTING)
        .await
        .map_err(|e| e.to_string())?;
    let folder: Option<String> = db
        .get_setting(crate::sync::FOLDER_SETTING)
        .await
        .map_err(|e| e.to_string())?;
    if server.is_some() && folder.is_none() {
        db.set_setting(crate::sync::FOLDER_SETTING, &OLD_SYNC_FOLDER)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::temp_dir;

    #[test]
    fn the_old_data_folder_moves_once_and_only_into_an_empty_new_one() {
        let root = temp_dir();
        let (old, new) = (
            root.join(OLD_IDENTIFIER),
            root.join("ai.agenticcraft.manent"),
        );
        std::fs::create_dir_all(old.join("meetings/1")).unwrap();
        std::fs::write(old.join(OLD_DATABASE), b"db").unwrap();
        std::fs::write(old.join("meetings/1/mix.opus"), b"audio").unwrap();
        // The webview may have made it already, with its caches.
        std::fs::create_dir_all(new.join("WebKitCache")).unwrap();
        assert_eq!(move_data_dir(&new).unwrap(), Some(old.clone()));
        assert!(!old.exists());
        assert_eq!(
            std::fs::read(new.join("meetings/1/mix.opus")).unwrap(),
            b"audio"
        );
        rename_database(&new, "manent.db").unwrap();
        assert!(new.join("manent.db").is_file() && !new.join(OLD_DATABASE).exists());
        // A second start, or Manent data already there: nothing moves.
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join(OLD_DATABASE), b"other").unwrap();
        assert_eq!(move_data_dir(&new).unwrap(), None);
        assert!(old.join(OLD_DATABASE).is_file());
    }

    #[tokio::test]
    async fn database_paths_follow_the_folder_and_sync_keeps_its_server_folder() {
        let db = Db::open_in_memory().await.unwrap();
        let m = db
            .create_meeting(
                "Planning",
                1,
                "/home/u/.local/share/nl.veritas.app/meetings/1",
            )
            .await
            .unwrap();
        let other = db
            .create_meeting("Elsewhere", 2, "/mnt/meetings/2")
            .await
            .unwrap();
        db.set_setting(crate::sync::SERVER_SETTING, &"https://cloud.example.org")
            .await
            .unwrap();
        after_open(
            &db,
            Some(Path::new("/home/u/.local/share/nl.veritas.app")),
            Path::new("/home/u/.local/share/ai.agenticcraft.manent"),
        )
        .await
        .unwrap();
        assert_eq!(
            db.get_meeting(m).await.unwrap().unwrap().dir,
            "/home/u/.local/share/ai.agenticcraft.manent/meetings/1"
        );
        assert_eq!(
            db.get_meeting(other).await.unwrap().unwrap().dir,
            "/mnt/meetings/2"
        );
        let folder: Option<String> = db.get_setting(crate::sync::FOLDER_SETTING).await.unwrap();
        assert_eq!(folder.as_deref(), Some(OLD_SYNC_FOLDER));
    }
}
