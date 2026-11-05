//! Small JSON files in the app's folder or a project's: read forgivingly,
//! written so a crash never leaves half a file.

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{AppError, AppResult};

/// The file's value, or the default when it is missing or damaged.
pub fn read_or_default<T: DeserializeOwned + Default>(file: &Path) -> T {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Written beside the file and renamed over it.
pub fn write_atomic<T: Serialize + ?Sized>(file: &Path, value: &T) -> AppResult<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|error| AppError::Io(error.to_string()))?;
    let draft = file.with_extension("json.tmp");
    std::fs::write(&draft, text)?;
    std::fs::rename(&draft, file)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_damaged_reads_as_default_and_a_write_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("deep").join("value.json");
        assert_eq!(read_or_default::<Vec<u32>>(&file), Vec::<u32>::new());
        write_atomic(&file, &vec![1, 2, 3]).unwrap();
        assert_eq!(read_or_default::<Vec<u32>>(&file), vec![1, 2, 3]);
        std::fs::write(&file, "{ broken").unwrap();
        assert_eq!(read_or_default::<Vec<u32>>(&file), Vec::<u32>::new());
        assert!(!file.with_extension("json.tmp").exists());
    }
}
