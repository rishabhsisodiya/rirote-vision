//! A still of each project for the Home screen: one engine snapshot, scaled
//! down and kept until the composition changes.

use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;

use crate::error::{AppError, AppResult};
use crate::{hyperframes, ids, preview, shell_env};

const SNAPSHOT_LIMIT: Duration = Duration::from_secs(90);
const SCALE_LIMIT: Duration = Duration::from_secs(30);
/// Wide enough for a Home card on a Retina screen.
const WIDTH: u32 = 360;

pub struct Posters {
    dir: PathBuf,
    /// One snapshot at a time: each starts a browser.
    lock: tokio::sync::Mutex<()>,
}

impl Posters {
    pub fn new(data_dir: &Path) -> Self {
        Self { dir: data_dir.join("posters"), lock: tokio::sync::Mutex::new(()) }
    }

    fn file(&self, project: &Path, signature: &str) -> PathBuf {
        self.dir.join(format!("{}-{signature}.jpg", ids::project_key(project)))
    }

    /// The project's poster as a `data:` URL, made first when the composition
    /// has changed since the last one.
    pub async fn get(&self, data_dir: &Path, project: &Path) -> AppResult<String> {
        if !project.join("index.html").is_file() {
            return Err(AppError::NotAProject("That folder has no composition to show.".into()));
        }
        let signature = preview::project_signature(project);
        let file = self.file(project, &signature);
        if !file.is_file() {
            let _guard = self.lock.lock().await;
            if !file.is_file() {
                self.make(data_dir, project, &file).await?;
            }
        }
        let bytes = std::fs::read(&file)?;
        Ok(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
    }

    async fn make(&self, data_dir: &Path, project: &Path, file: &Path) -> AppResult<()> {
        let scratch = tempfile_dir(&self.dir)?;
        let out = scratch.to_string_lossy().into_owned();
        // A third of the way in is usually past the intro and before the end card.
        let at = composition_seconds(project).map_or(1.0, |seconds| (seconds / 3.0).max(0.5));
        let at = format!("{at:.2}");
        let output = hyperframes::run(
            data_dir,
            &["snapshot", "--at", &at, "--no-end", "--json", "-o", &out],
            project,
            SNAPSHOT_LIMIT,
        )
        .await;
        let still = std::fs::read_dir(&scratch)
            .ok()
            .and_then(|entries| {
                entries.filter_map(Result::ok).map(|entry| entry.path()).find(|path| {
                    path.extension().is_some_and(|ext| ext == "png")
                })
            });
        let result = match (output, still) {
            (_, Some(still)) => scale(&still, file).await,
            (Ok(output), None) => Err(AppError::ToolFailed(format!(
                "The engine could not take a still of this project.\n{}",
                shell_env::failure_text(&output)
            ))),
            (Err(error), None) => Err(error),
        };
        let _ = std::fs::remove_dir_all(&scratch);
        // Older posters of the same project are no longer needed.
        if result.is_ok() {
            remove_older(&self.dir, file);
        }
        result
    }
}

fn tempfile_dir(parent: &Path) -> AppResult<PathBuf> {
    std::fs::create_dir_all(parent)?;
    let dir = parent.join(format!(".snap-{}-{}", std::process::id(), nanos()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn nanos() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |time| time.as_nanos())
}

/// The composition's length from its root `data-duration`, when it says.
fn composition_seconds(project: &Path) -> Option<f64> {
    let html = std::fs::read_to_string(project.join("index.html")).ok()?;
    let at = html.find("data-duration=\"")? + "data-duration=\"".len();
    html[at..].split('"').next()?.trim().parse().ok().filter(|seconds: &f64| *seconds > 0.0)
}

async fn scale(still: &Path, file: &Path) -> AppResult<()> {
    let ffmpeg = shell_env::require_program("ffmpeg", "Install FFmpeg with `brew install ffmpeg`.")?;
    let mut command = shell_env::command(&ffmpeg);
    command
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(still)
        .args(["-vf", &format!("scale={WIDTH}:-2"), "-q:v", "4"])
        .arg(file);
    let output = shell_env::run(command, SCALE_LIMIT, "ffmpeg").await?;
    if !output.status.success() || !file.is_file() {
        return Err(AppError::ToolFailed(format!(
            "The still could not be scaled.\n{}",
            shell_env::failure_text(&output)
        )));
    }
    Ok(())
}

/// Removes the project's other posters, keeping `keep`.
fn remove_older(dir: &Path, keep: &Path) {
    let Some(name) = keep.file_name().and_then(|name| name.to_str()) else { return };
    let Some((key, _)) = name.split_once('-') else { return };
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let other = entry.file_name().to_string_lossy().into_owned();
        if path != keep && other.starts_with(&format!("{key}-")) {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_root_duration() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), r#"<div data-composition-id="x" data-duration="24.5">"#).unwrap();
        assert_eq!(composition_seconds(dir.path()), Some(24.5));
        std::fs::write(dir.path().join("index.html"), "<div>").unwrap();
        assert_eq!(composition_seconds(dir.path()), None);
    }

    #[test]
    fn only_the_same_projects_older_posters_go() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["aaaa-1.jpg", "aaaa-2.jpg", "bbbb-1.jpg"] {
            std::fs::write(dir.path().join(name), "x").unwrap();
        }
        remove_older(dir.path(), &dir.path().join("aaaa-2.jpg"));
        let mut left: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["aaaa-2.jpg", "bbbb-1.jpg"]);
    }
}
