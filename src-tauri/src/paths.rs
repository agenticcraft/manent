//! Where Manent keeps its data: the database and one folder per meeting.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Environment override for the data folder (tests and the UI smoke check use a temp folder).
pub const DATA_DIR_ENV: &str = "MANENT_DATA_DIR";
/// The database file in the data folder (it was veritas.db before VRT-44).
pub const DATABASE_FILE: &str = "manent.db";

/// Settings key of a meetings folder outside the data folder (Settings > Storage, VRT-27).
pub const MEETINGS_DIR_SETTING: &str = "storage.meetings_dir";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data: PathBuf,
    /// A chosen meetings folder; shared by every copy, so a move applies everywhere at once.
    meetings: Arc<RwLock<Option<PathBuf>>>,
}

impl AppPaths {
    pub fn new(data: PathBuf) -> Self {
        Self {
            data,
            meetings: Arc::default(),
        }
    }

    /// `MANENT_DATA_DIR` if set, otherwise the OS app data folder.
    pub fn resolve(app_data_dir: PathBuf) -> Self {
        let data = crate::legacy::env_var(DATA_DIR_ENV.trim_start_matches("MANENT_"))
            .map(PathBuf::from)
            .unwrap_or(app_data_dir);
        Self::new(data)
    }

    /// Use another meetings folder from now on (None: the default in the data folder).
    pub fn set_meetings_root(&self, dir: Option<PathBuf>) {
        *self.meetings.write().unwrap_or_else(|e| e.into_inner()) = dir;
    }

    pub fn database(&self) -> PathBuf {
        self.data.join(DATABASE_FILE)
    }

    /// Where the meeting folders are: the chosen folder, or `<data>/meetings`.
    pub fn meetings_root(&self) -> PathBuf {
        self.meetings
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .unwrap_or_else(|| self.data.join("meetings"))
    }

    /// Downloaded engine library (libparakeet and its OpenMP runtime), VRT-14.
    pub fn engine_dir(&self) -> PathBuf {
        self.data.join("engine")
    }

    /// Short-lived audio handed to the engine (live chunks), VRT-15.
    pub fn tmp_dir(&self) -> PathBuf {
        self.data.join("tmp")
    }

    /// Downloaded models, VRT-14.
    pub fn models_dir(&self) -> PathBuf {
        self.data.join("models")
    }

    /// A meeting folder the app may read or delete: it must exist and lie strictly inside the
    /// meetings root (after resolving symlinks and `..`). Anything else returns None.
    pub fn contained_meeting_dir(&self, dir: &Path) -> Option<PathBuf> {
        let root = self.meetings_root().canonicalize().ok()?;
        let dir = dir.canonicalize().ok()?;
        (dir != root && dir.starts_with(&root)).then_some(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_folders_inside_the_meetings_root_are_accepted() {
        let data = crate::test_util::temp_dir();
        let paths = AppPaths::new(data.clone());
        let meeting = paths.meetings_root().join("42");
        std::fs::create_dir_all(&meeting).unwrap();
        let outside = data.join("elsewhere");
        std::fs::create_dir_all(&outside).unwrap();

        assert_eq!(
            paths.contained_meeting_dir(&meeting),
            Some(meeting.canonicalize().unwrap())
        );
        assert_eq!(paths.contained_meeting_dir(&outside), None);
        assert_eq!(
            paths.contained_meeting_dir(&paths.meetings_root()),
            None,
            "not the root itself"
        );
        assert_eq!(
            paths.contained_meeting_dir(&meeting.join("../../elsewhere")),
            None,
            "no .. escape"
        );
        assert_eq!(
            paths.contained_meeting_dir(&paths.meetings_root().join("missing")),
            None
        );
        assert_eq!(paths.contained_meeting_dir(Path::new("/")), None);
    }
}

/// Move every meeting folder from `from` to `to` (rename, or copy and delete across file
/// systems). Returns (old folder, new folder) per meeting moved. `to` must not lie inside
/// `from` or the other way round.
pub fn move_meeting_folders(from: &Path, to: &Path) -> std::io::Result<Vec<(PathBuf, PathBuf)>> {
    // Check before creating anything, so a refused move leaves no folder behind.
    let (f, t) = (from.canonicalize()?, resolved(to)?);
    if f == t || t.starts_with(&f) || f.starts_with(&t) {
        return Err(std::io::Error::other(
            "the new folder can't be the old one or lie inside it (or the other way round)",
        ));
    }
    std::fs::create_dir_all(&t)?;
    let mut moved = Vec::new();
    for entry in std::fs::read_dir(&f)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let (old, new) = (entry.path(), t.join(entry.file_name()));
        if new.exists() {
            return Err(std::io::Error::other(format!(
                "{} already exists",
                new.display()
            )));
        }
        if std::fs::rename(&old, &new).is_err() {
            copy_dir(&old, &new)?;
            std::fs::remove_dir_all(&old)?;
        }
        moved.push((old, new));
    }
    Ok(moved)
}

/// `path` with symlinks and `..` resolved, also when it doesn't exist yet.
fn resolved(path: &Path) -> std::io::Result<PathBuf> {
    match path.canonicalize() {
        Ok(p) => Ok(p),
        Err(_) => {
            let parent = path
                .parent()
                .ok_or_else(|| std::io::Error::other("no parent folder"))?;
            let name = path
                .file_name()
                .ok_or_else(|| std::io::Error::other("no folder name"))?;
            Ok(resolved(parent)?.join(name))
        }
    }
}

pub(crate) fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod move_tests {
    use super::*;

    #[test]
    fn meetings_root_can_change_for_every_copy() {
        let paths = AppPaths::new(crate::test_util::temp_dir());
        let copy = paths.clone();
        assert_eq!(copy.meetings_root(), paths.data.join("meetings"));
        let elsewhere = crate::test_util::temp_dir().join("Meetings");
        paths.set_meetings_root(Some(elsewhere.clone()));
        assert_eq!(
            copy.meetings_root(),
            elsewhere,
            "copies (media server, jobs) follow"
        );
        paths.set_meetings_root(None);
        assert_eq!(copy.meetings_root(), paths.data.join("meetings"));
    }

    #[test]
    fn moves_meeting_folders_and_refuses_nested_targets() {
        let from = crate::test_util::temp_dir().join("meetings");
        for id in ["1", "2"] {
            std::fs::create_dir_all(from.join(id).join("sub")).unwrap();
            std::fs::write(from.join(id).join("mic.flac"), id).unwrap();
            std::fs::write(from.join(id).join("sub/x"), b"x").unwrap();
        }
        std::fs::write(from.join("stray.txt"), b"not a meeting").unwrap();
        assert!(move_meeting_folders(&from, &from.join("inner")).is_err());
        assert!(
            !from.join("inner").exists(),
            "a refused move creates nothing"
        );
        let to = crate::test_util::temp_dir().join("elsewhere");
        let mut moved = move_meeting_folders(&from, &to).unwrap();
        moved.sort();
        assert_eq!(moved.len(), 2);
        assert_eq!(std::fs::read_to_string(to.join("2/mic.flac")).unwrap(), "2");
        assert!(to.join("1/sub/x").is_file() && !from.join("1").exists());
        assert!(from.join("stray.txt").exists(), "only meeting folders move");
        // A clash leaves things as they are.
        std::fs::create_dir_all(from.join("1")).unwrap();
        assert!(move_meeting_folders(&from, &to).is_err());
    }

    #[test]
    fn copy_dir_copies_nested_files() {
        let from = crate::test_util::temp_dir();
        std::fs::create_dir_all(from.join("a/b")).unwrap();
        std::fs::write(from.join("a/b/c"), b"c").unwrap();
        let to = crate::test_util::temp_dir().join("copy");
        copy_dir(&from, &to).unwrap();
        assert_eq!(std::fs::read(to.join("a/b/c")).unwrap(), b"c");
    }
}
