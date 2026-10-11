//! The project's media: the picture, sound and caption files it holds, and
//! bringing new ones in.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use serde::Serialize;
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::shell_env;

/// Folders that never hold the user's media.
const SKIPPED_DIRS: [&str; 5] = ["node_modules", "snapshots", "renders", "out", "dist"];
const MAX_DEPTH: usize = 6;
const MAX_ITEMS: usize = 2_000;
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Video,
    Image,
    Audio,
    Captions,
}

impl MediaKind {
    pub fn of(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        Some(match extension.as_str() {
            "mp4" | "mov" | "webm" | "mkv" | "m4v" => Self::Video,
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "avif" => Self::Image,
            "wav" | "mp3" | "m4a" | "aac" | "ogg" | "flac" => Self::Audio,
            "srt" | "vtt" => Self::Captions,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    /// Where the file is inside the project, with forward slashes.
    pub path: String,
    pub name: String,
    pub kind: MediaKind,
    pub bytes: u64,
    /// Length in seconds, for sound and video that could be measured.
    pub duration: Option<f64>,
}

/// Remembers measured lengths so a file is probed once, not on every listing.
#[derive(Default)]
pub struct MediaLibrary {
    durations: Mutex<HashMap<(PathBuf, u64, Option<SystemTime>), Option<f64>>>,
}

impl MediaLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every media file in the project, by kind and then by name.
    pub async fn list(&self, project: &Path) -> AppResult<Vec<MediaItem>> {
        let mut files = Vec::new();
        collect(project, project, 0, &mut files)?;
        let mut items = Vec::with_capacity(files.len());
        for (path, kind) in files {
            items.push(self.describe(project, &path, kind).await?);
        }
        items.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase())));
        Ok(items)
    }

    async fn describe(&self, project: &Path, path: &Path, kind: MediaKind) -> AppResult<MediaItem> {
        let meta = std::fs::metadata(path)?;
        let duration = match kind {
            MediaKind::Audio | MediaKind::Video => {
                let key = (path.to_path_buf(), meta.len(), meta.modified().ok());
                let known = self.durations.lock().ok().and_then(|cache| cache.get(&key).copied());
                match known {
                    Some(duration) => duration,
                    None => {
                        let duration = probe_duration(path).await;
                        if let Ok(mut cache) = self.durations.lock() {
                            cache.insert(key, duration);
                        }
                        duration
                    }
                }
            }
            _ => None,
        };
        Ok(MediaItem {
            path: relative(project, path),
            name: path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
            kind,
            bytes: meta.len(),
            duration,
        })
    }

    /// Copies files into the project's assets folder and describes the copies.
    /// Files of a kind the editor cannot use are refused before anything is copied.
    pub async fn import(&self, project: &Path, sources: &[PathBuf]) -> AppResult<Vec<MediaItem>> {
        if sources.is_empty() {
            return Ok(Vec::new());
        }
        let mut planned = Vec::new();
        for source in sources {
            if !source.is_file() {
                return Err(AppError::InvalidInput(format!("{} is not a file.", source.display())));
            }
            let kind = MediaKind::of(source).ok_or_else(|| {
                AppError::InvalidInput(format!(
                    "{} is not a kind of media the editor can use.",
                    source.file_name().unwrap_or_default().to_string_lossy()
                ))
            })?;
            planned.push((source, kind));
        }

        let assets = assets_dir(project);
        std::fs::create_dir_all(&assets)?;
        let mut imported = Vec::new();
        for (source, kind) in planned {
            // A file that is already in the project is listed, not copied again.
            let target = if source.starts_with(project) { source.clone() } else { free_name(&assets, source) };
            if &target != source {
                tokio::fs::copy(source, &target).await?;
            }
            imported.push(self.describe(project, &target, kind).await?);
        }
        Ok(imported)
    }
}

/// The folder new media goes into: the one hyperframes.json names, else `assets`.
pub fn assets_dir(project: &Path) -> PathBuf {
    let named = std::fs::read_to_string(project.join("hyperframes.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|config| config["paths"]["assets"].as_str().map(str::to_owned))
        .filter(|name| resolve(project, name).is_ok());
    project.join(named.unwrap_or_else(|| "assets".into()))
}

/// The file a project-relative path names. Refuses anything that would leave
/// the project.
pub fn resolve(project: &Path, relative: &str) -> AppResult<PathBuf> {
    let path = Path::new(relative);
    let plain = !relative.is_empty()
        && path.components().all(|part| matches!(part, Component::Normal(_)));
    if plain {
        Ok(project.join(path))
    } else {
        Err(AppError::InvalidInput("That is not a path inside the project.".into()))
    }
}

/// A path for `source`'s name in `dir` that nothing is using: `a.png`, then
/// `a-2.png`, `a-3.png` and so on.
pub fn free_name(dir: &Path, source: &Path) -> PathBuf {
    let stem = source.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
    let extension = source.extension().map(|ext| format!(".{}", ext.to_string_lossy())).unwrap_or_default();
    let mut target = dir.join(format!("{stem}{extension}"));
    let mut number = 2;
    while target.exists() {
        target = dir.join(format!("{stem}-{number}{extension}"));
        number += 1;
    }
    target
}

fn relative(project: &Path, path: &Path) -> String {
    let inside = path.strip_prefix(project).unwrap_or(path);
    inside.components().map(|part| part.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

fn collect(project: &Path, dir: &Path, depth: usize, found: &mut Vec<(PathBuf, MediaKind)>) -> AppResult<()> {
    if depth > MAX_DEPTH {
        return Ok(());
    }
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.filter_map(Result::ok).collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if found.len() >= MAX_ITEMS {
            break;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else { continue };
        let path = entry.path();
        if kind.is_dir() {
            if !name.starts_with('.') && !SKIPPED_DIRS.contains(&name.as_str()) {
                collect(project, &path, depth + 1, found)?;
            }
        } else if kind.is_file() && !name.starts_with('.') {
            if let Some(media) = MediaKind::of(&path) {
                found.push((path, media));
            }
        }
    }
    Ok(())
}

/// A file's length from ffprobe, when ffprobe is there and can read it.
async fn probe_duration(path: &Path) -> Option<f64> {
    let ffprobe = shell_env::find_program("ffprobe")?;
    let mut command = shell_env::command(&ffprobe);
    command
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=noprint_wrappers=1:nokey=1"])
        .arg(path);
    let output = shell_env::run(command, PROBE_TIMEOUT, "ffprobe").await.ok()?;
    let seconds: f64 = String::from_utf8_lossy(&output.stdout).trim().parse().ok()?;
    (seconds.is_finite() && seconds > 0.0).then_some(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"x").unwrap();
    }

    #[test]
    fn kinds_come_from_the_extension() {
        assert_eq!(MediaKind::of(Path::new("a/Clip.MP4")), Some(MediaKind::Video));
        assert_eq!(MediaKind::of(Path::new("logo.svg")), Some(MediaKind::Image));
        assert_eq!(MediaKind::of(Path::new("vo.wav")), Some(MediaKind::Audio));
        assert_eq!(MediaKind::of(Path::new("subs.srt")), Some(MediaKind::Captions));
        assert_eq!(MediaKind::of(Path::new("index.html")), None);
        assert_eq!(MediaKind::of(Path::new("noext")), None);
    }

    #[tokio::test]
    async fn listing_finds_media_and_skips_what_is_not_the_users() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path();
        for file in [
            "assets/b.png",
            "assets/A.png",
            "assets/vo/line.srt",
            "index.html",
            "node_modules/pkg/icon.png",
            ".hyperframes/backup/x.png",
            "snapshots/frame.png",
            "assets/.hidden.png",
        ] {
            touch(&project.join(file));
        }
        let items = MediaLibrary::new().list(project).await.unwrap();
        let paths: Vec<_> = items.iter().map(|item| item.path.as_str()).collect();
        assert_eq!(paths, ["assets/A.png", "assets/b.png", "assets/vo/line.srt"]);
        assert_eq!(items[0].bytes, 1);
        assert_eq!(items[2].kind, MediaKind::Captions);
    }

    #[tokio::test]
    async fn importing_copies_into_assets_without_overwriting() {
        let outside = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path();
        let source = outside.path().join("shot.png");
        touch(&source);
        touch(&project.join("assets/shot.png"));

        let library = MediaLibrary::new();
        let first = library.import(project, &[source.clone()]).await.unwrap();
        assert_eq!(first[0].path, "assets/shot-2.png");
        let second = library.import(project, &[source.clone()]).await.unwrap();
        assert_eq!(second[0].path, "assets/shot-3.png");
        assert!(source.exists(), "the original stays where it was");

        let inside = library.import(project, &[project.join("assets/shot.png")]).await.unwrap();
        assert_eq!(inside[0].path, "assets/shot.png", "a project file is not copied again");
    }

    #[tokio::test]
    async fn importing_refuses_other_files_before_copying_anything() {
        let outside = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let (good, bad) = (outside.path().join("a.png"), outside.path().join("notes.txt"));
        touch(&good);
        touch(&bad);
        let library = MediaLibrary::new();
        let result = library.import(dir.path(), &[good, bad]).await;
        assert_eq!(result.unwrap_err().code(), "invalid_input");
        assert!(!dir.path().join("assets/a.png").exists());
        let missing = library.import(dir.path(), &[outside.path().join("gone.png")]).await;
        assert_eq!(missing.unwrap_err().code(), "invalid_input");
    }

    #[test]
    fn the_assets_folder_follows_the_project_file() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(assets_dir(dir.path()), dir.path().join("assets"));
        std::fs::write(dir.path().join("hyperframes.json"), r#"{"paths":{"assets":"media"}}"#).unwrap();
        assert_eq!(assets_dir(dir.path()), dir.path().join("media"));
        std::fs::write(dir.path().join("hyperframes.json"), r#"{"paths":{"assets":"../elsewhere"}}"#).unwrap();
        assert_eq!(assets_dir(dir.path()), dir.path().join("assets"));
    }

    #[test]
    fn paths_must_stay_in_the_project() {
        let project = Path::new("/films/a");
        assert_eq!(resolve(project, "assets/vo/line.wav").unwrap(), project.join("assets/vo/line.wav"));
        for bad in ["", "../b/x.png", "/etc/passwd", "assets/../../x", "./x.png"] {
            assert!(resolve(project, bad).is_err(), "{bad}");
        }
    }
}
