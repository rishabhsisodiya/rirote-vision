//! The app's own log: one line per event in `logs/app.log`, kept small.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_BYTES: u64 = 1024 * 1024;

pub fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("logs")
}

fn file(data_dir: &Path) -> PathBuf {
    dir(data_dir).join("app.log")
}

/// Appends a line. A log that cannot be written is never a reason to fail.
pub fn write(data_dir: &Path, source: &str, message: &str) {
    let path = file(data_dir);
    let _ = std::fs::create_dir_all(dir(data_dir));
    if std::fs::metadata(&path).map(|meta| meta.len() > MAX_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(&path, path.with_extension("log.old"));
    }
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map(|t| t.as_secs()).unwrap_or(0);
    let line = message.replace('\n', " ⏎ ");
    if let Ok(mut log) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(log, "{seconds} [{source}] {line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_appended_one_per_event() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "test", "first");
        write(dir.path(), "test", "second\nline");
        let text = std::fs::read_to_string(file(dir.path())).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with("[test] first"));
        assert!(lines[1].ends_with("[test] second ⏎ line"));
    }
}
