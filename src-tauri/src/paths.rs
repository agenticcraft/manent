//! Where Veritas keeps its data: the database and one folder per meeting.

use std::path::{Path, PathBuf};

/// Environment override for the data folder (tests and the UI smoke check use a temp folder).
pub const DATA_DIR_ENV: &str = "VERITAS_DATA_DIR";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data: PathBuf,
}

impl AppPaths {
    /// `VERITAS_DATA_DIR` if set, otherwise the OS app data folder.
    pub fn resolve(app_data_dir: PathBuf) -> Self {
        let data = std::env::var_os(DATA_DIR_ENV)
            .map(PathBuf::from)
            .unwrap_or(app_data_dir);
        Self { data }
    }

    pub fn database(&self) -> PathBuf {
        self.data.join("veritas.db")
    }

    pub fn meetings_root(&self) -> PathBuf {
        self.data.join("meetings")
    }

    /// A meeting folder that may be deleted: it must exist and lie strictly inside the
    /// meetings root (after resolving symlinks and `..`). Anything else returns None.
    pub fn deletable_meeting_dir(&self, dir: &Path) -> Option<PathBuf> {
        let root = self.meetings_root().canonicalize().ok()?;
        let dir = dir.canonicalize().ok()?;
        (dir != root && dir.starts_with(&root)).then_some(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_folders_inside_the_meetings_root_are_deletable() {
        let data = crate::test_util::temp_dir();
        let paths = AppPaths { data: data.clone() };
        let meeting = paths.meetings_root().join("42");
        std::fs::create_dir_all(&meeting).unwrap();
        let outside = data.join("elsewhere");
        std::fs::create_dir_all(&outside).unwrap();

        assert_eq!(
            paths.deletable_meeting_dir(&meeting),
            Some(meeting.canonicalize().unwrap())
        );
        assert_eq!(paths.deletable_meeting_dir(&outside), None);
        assert_eq!(
            paths.deletable_meeting_dir(&paths.meetings_root()),
            None,
            "not the root itself"
        );
        assert_eq!(
            paths.deletable_meeting_dir(&meeting.join("../../elsewhere")),
            None,
            "no .. escape"
        );
        assert_eq!(
            paths.deletable_meeting_dir(&paths.meetings_root().join("missing")),
            None
        );
        assert_eq!(paths.deletable_meeting_dir(Path::new("/")), None);
    }
}
