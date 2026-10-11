//! The list of recently opened projects, kept as one small JSON file in the
//! app's folder.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::project::ProjectSummary;

const MAX_RECENTS: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Stored {
    path: String,
    name: String,
    /// Seconds since 1970.
    opened_at: u64,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecentProject {
    pub path: String,
    pub name: String,
    pub opened_at: u64,
    /// The folder has been moved or deleted since it was last opened.
    pub missing: bool,
}

pub struct RecentStore {
    file: PathBuf,
    /// Serialises read-modify-write cycles on the file.
    lock: Mutex<()>,
}

impl RecentStore {
    pub fn new(data_dir: &Path) -> Self {
        Self { file: data_dir.join("recent-projects.json"), lock: Mutex::new(()) }
    }

    /// Newest first.
    pub fn list(&self) -> Vec<RecentProject> {
        let _guard = self.lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        self.read()
            .into_iter()
            .map(|entry| RecentProject {
                missing: !Path::new(&entry.path).is_dir(),
                path: entry.path,
                name: entry.name,
                opened_at: entry.opened_at,
            })
            .collect()
    }

    /// Moves `project` to the top of the list.
    pub fn touch(&self, project: &ProjectSummary) -> AppResult<()> {
        let _guard = self.lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut entries = self.read();
        entries.retain(|entry| entry.path != project.path);
        entries.insert(
            0,
            Stored { path: project.path.clone(), name: project.name.clone(), opened_at: now() },
        );
        entries.truncate(MAX_RECENTS);
        self.write(&entries)
    }

    /// Removes a project from the list. The folder itself is untouched.
    pub fn forget(&self, path: &str) -> AppResult<()> {
        let _guard = self.lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut entries = self.read();
        entries.retain(|entry| entry.path != path);
        self.write(&entries)
    }

    /// An unreadable or damaged file counts as an empty list.
    fn read(&self) -> Vec<Stored> {
        std::fs::read_to_string(&self.file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Written beside the file and renamed over it, so a crash never leaves
    /// half a list.
    fn write(&self, entries: &[Stored]) -> AppResult<()> {
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(entries)
            .map_err(|error| crate::error::AppError::Io(error.to_string()))?;
        let draft = self.file.with_extension("json.tmp");
        std::fs::write(&draft, text)?;
        std::fs::rename(&draft, &self.file)?;
        Ok(())
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(path: &Path, name: &str) -> ProjectSummary {
        ProjectSummary { path: path.to_string_lossy().into_owned(), name: name.into() }
    }

    #[test]
    fn the_last_opened_project_comes_first_and_is_listed_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = RecentStore::new(dir.path());
        let a = summary(dir.path(), "a");
        let b = summary(&dir.path().join("b"), "b");
        store.touch(&a).unwrap();
        store.touch(&b).unwrap();
        store.touch(&a).unwrap();

        let names: Vec<_> = store.list().into_iter().map(|entry| entry.name).collect();
        assert_eq!(names, ["a", "b"]);
    }

    #[test]
    fn a_moved_folder_is_marked_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = RecentStore::new(dir.path());
        store.touch(&summary(&dir.path().join("gone"), "gone")).unwrap();
        store.touch(&summary(dir.path(), "here")).unwrap();

        let list = store.list();
        assert!(!list[0].missing);
        assert!(list[1].missing);
    }

    #[test]
    fn the_list_is_capped() {
        let dir = tempfile::tempdir().unwrap();
        let store = RecentStore::new(dir.path());
        for index in 0..MAX_RECENTS + 5 {
            store.touch(&summary(&dir.path().join(index.to_string()), "p")).unwrap();
        }
        assert_eq!(store.list().len(), MAX_RECENTS);
    }

    #[test]
    fn forgetting_removes_only_that_project() {
        let dir = tempfile::tempdir().unwrap();
        let store = RecentStore::new(dir.path());
        let a = summary(&dir.path().join("a"), "a");
        store.touch(&a).unwrap();
        store.touch(&summary(&dir.path().join("b"), "b")).unwrap();
        store.forget(&a.path).unwrap();

        let names: Vec<_> = store.list().into_iter().map(|entry| entry.name).collect();
        assert_eq!(names, ["b"]);
    }

    #[test]
    fn a_damaged_file_reads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("recent-projects.json"), "{not json").unwrap();
        assert!(RecentStore::new(dir.path()).list().is_empty());
    }
}
