//! The app's own Python environment, used for voiceover and for music.
//!
//! The engine's text-to-speech needs the Kokoro packages, and looks for them
//! in whatever `python3` it finds, which on most machines does not have
//! them. The music synth needs numpy, which comes with them. Keeping an
//! environment in the app's own folder means both work without touching the
//! user's Python.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::{AppError, AppResult};
use crate::shell_env;

const PACKAGES: [&str; 2] = ["kokoro-onnx", "soundfile"];
/// Python versions the packages are published for, newest first.
const SUPPORTED_MINORS: [u32; 4] = [12, 11, 13, 10];
const QUICK_TIMEOUT: Duration = Duration::from_secs(20);
const INSTALL_TIMEOUT: Duration = Duration::from_secs(1200);

fn env_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("voice-env")
}

/// The environment's Python, once it has been installed.
pub fn python(data_dir: &Path) -> Option<PathBuf> {
    let python = env_dir(data_dir).join("bin").join("python");
    python.is_file().then_some(python)
}

/// A Python on this Mac that the packages support.
async fn base_python() -> AppResult<PathBuf> {
    let mut names: Vec<String> = SUPPORTED_MINORS.iter().map(|minor| format!("python3.{minor}")).collect();
    names.push("python3".into());
    for name in names {
        let Some(candidate) = shell_env::find_program(&name) else { continue };
        let mut command = shell_env::command(&candidate);
        command.args(["-c", "import sys; print(sys.version_info[0], sys.version_info[1])"]);
        let Ok(output) = shell_env::run(command, QUICK_TIMEOUT, "python").await else { continue };
        if supported(&String::from_utf8_lossy(&output.stdout)) {
            return Ok(candidate);
        }
    }
    Err(AppError::ToolMissing(
        "Voiceover and music need Python 3.10 to 3.13, and none was found. Install one (for example `brew install python@3.12`), then try again.".into(),
    ))
}

/// True for the output of the version probe when it names a supported Python.
fn supported(version: &str) -> bool {
    let mut parts = version.split_whitespace().map(str::parse::<u32>);
    matches!((parts.next(), parts.next()), (Some(Ok(3)), Some(Ok(minor))) if SUPPORTED_MINORS.contains(&minor))
}

/// Creates the environment and downloads the packages into it.
pub async fn install(data_dir: &Path) -> AppResult<()> {
    let base = base_python().await?;
    let dir = env_dir(data_dir);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }

    let mut create = shell_env::command(&base);
    create.args(["-m", "venv"]).arg(&dir);
    step(create, QUICK_TIMEOUT * 6, "Creating the voiceover and music environment").await?;

    let python = python(data_dir)
        .ok_or_else(|| AppError::ToolFailed("The voiceover and music environment was not created.".into()))?;
    let mut pip = shell_env::command(&python);
    pip.args(["-m", "pip", "install", "--disable-pip-version-check", "--quiet"]).args(PACKAGES);
    if let Err(error) = step(pip, INSTALL_TIMEOUT, "Downloading the voiceover and music packages").await {
        // Half an environment would look installed; remove it.
        let _ = std::fs::remove_dir_all(&dir);
        return Err(error);
    }
    Ok(())
}

async fn step(command: tokio::process::Command, limit: Duration, what: &str) -> AppResult<()> {
    let output = shell_env::run(command, limit, what).await?;
    if output.status.success() {
        Ok(())
    } else {
        Err(AppError::ToolFailed(format!("{what} failed.\n{}", shell_env::failure_text(&output))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_supported_pythons_are_used() {
        assert!(supported("3 12\n"));
        assert!(supported("3 10"));
        assert!(!supported("3 14"));
        assert!(!supported("3 9"));
        assert!(!supported("2 7"));
        assert!(!supported("nonsense"));
    }

    #[test]
    fn no_environment_means_no_python() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(python(dir.path()), None);
    }
}
