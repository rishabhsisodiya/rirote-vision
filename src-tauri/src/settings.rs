//! The app's own preferences, one JSON file in its folder. Every field has a
//! default, so a file from an older version still reads.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::store;

/// Model aliases Claude Code accepts for `--model`; empty means its own default.
pub const MODELS: [&str; 4] = ["", "fable", "opus", "sonnet"];
pub const EFFORTS: [&str; 6] = ["", "low", "medium", "high", "xhigh", "max"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// `--model` for assistant turns; empty leaves Claude Code's choice.
    pub assistant_model: String,
    /// `--effort` for assistant turns; empty leaves Claude Code's choice.
    pub assistant_effort: String,
    /// Longest reference video the app will download and study, in minutes.
    pub reference_max_minutes: u32,
    /// Where Home's Make and Script start new projects.
    pub projects_folder: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            assistant_model: String::new(),
            assistant_effort: String::new(),
            reference_max_minutes: 10,
            projects_folder: default_projects_folder(),
        }
    }
}

fn default_projects_folder() -> String {
    std::env::var("HOME")
        .map(|home| format!("{home}/Movies/RIROTE VISION"))
        .unwrap_or_default()
}

impl Settings {
    fn validate(&self) -> AppResult<()> {
        if !MODELS.contains(&self.assistant_model.as_str()) {
            return Err(AppError::InvalidInput(format!("Unknown model “{}”.", self.assistant_model)));
        }
        if !EFFORTS.contains(&self.assistant_effort.as_str()) {
            return Err(AppError::InvalidInput(format!("Unknown effort “{}”.", self.assistant_effort)));
        }
        if !Path::new(&self.projects_folder).is_absolute() {
            return Err(AppError::InvalidInput("Choose a folder for new projects.".into()));
        }
        if !(1..=30).contains(&self.reference_max_minutes) {
            return Err(AppError::InvalidInput("Reference length must be 1 to 30 minutes.".into()));
        }
        Ok(())
    }
}

pub struct SettingsStore {
    file: PathBuf,
    lock: Mutex<()>,
}

impl SettingsStore {
    pub fn new(data_dir: &Path) -> Self {
        Self { file: data_dir.join("settings.json"), lock: Mutex::new(()) }
    }

    pub fn get(&self) -> Settings {
        let _guard = self.lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        store::read_or_default(&self.file)
    }

    /// The folder for new projects, made if it is not there yet.
    pub fn projects_folder(&self) -> AppResult<PathBuf> {
        let mut folder = PathBuf::from(self.get().projects_folder);
        if !folder.is_absolute() {
            folder = PathBuf::from(default_projects_folder());
        }
        std::fs::create_dir_all(&folder)?;
        Ok(folder)
    }

    pub fn set(&self, settings: Settings) -> AppResult<Settings> {
        settings.validate()?;
        let _guard = self.lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        store::write_atomic(&self.file, &settings)?;
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_fill_missing_fields_and_bad_values_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), r#"{"assistantModel":"opus"}"#).unwrap();
        let store = SettingsStore::new(dir.path());
        let read = store.get();
        assert_eq!(read.assistant_model, "opus");
        assert_eq!(read.reference_max_minutes, 10);
        // Fields this version no longer has, like the old Instagram search, are ignored.
        std::fs::write(dir.path().join("settings.json"), r#"{"instagramSearch":true,"cookieBrowser":"safari"}"#).unwrap();
        assert_eq!(store.get(), Settings::default());
        std::fs::write(dir.path().join("settings.json"), r#"{"assistantModel":"opus"}"#).unwrap();

        let bad = Settings { assistant_model: "gpt".into(), ..Settings::default() };
        assert_eq!(store.set(bad).unwrap_err().code(), "invalid_input");
        let long = Settings { reference_max_minutes: 90, ..Settings::default() };
        assert_eq!(store.set(long).unwrap_err().code(), "invalid_input");

        let good = Settings { reference_max_minutes: 20, ..Settings::default() };
        store.set(good.clone()).unwrap();
        assert_eq!(store.get(), good);
    }
}
