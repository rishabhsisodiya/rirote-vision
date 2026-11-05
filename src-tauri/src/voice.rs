//! The app's own Python environment, used for voiceover and for music.
//!
//! The engine's text-to-speech needs the Kokoro packages, and the music synth
//! needs numpy, which comes with them. Many Macs have no suitable Python, so
//! the app downloads its own: a standalone CPython build (python-build-standalone,
//! pinned and checked against its SHA-256), kept in the app's folder. The
//! environment is made from it, and nothing on the user's Python is touched.
//! The voice model itself (Kokoro, about 340 MB, male and female voices) is
//! fetched by the engine on first use; setup speaks one sample to fetch it then.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::{AppError, AppResult};
use crate::{hyperframes, shell_env};

const PACKAGES: [&str; 2] = ["kokoro-onnx", "soundfile"];
const QUICK_TIMEOUT: Duration = Duration::from_secs(20);
const INSTALL_TIMEOUT: Duration = Duration::from_secs(1200);
const RUNTIME_TIMEOUT: Duration = Duration::from_secs(600);
const SAMPLE_TIMEOUT: Duration = Duration::from_secs(1200);

/// The standalone Python the app downloads, by release, file and checksum.
const RUNTIME_RELEASE: &str = "https://github.com/astral-sh/python-build-standalone/releases/download/20261021";
#[cfg(target_arch = "aarch64")]
const RUNTIME_ASSET: (&str, &str) = (
    "cpython-3.12.15+20261021-aarch64-apple-darwin-install_only_stripped.tar.gz",
    "7b8d00b751db21cea87dc551a8f8165147c5fbc5251d5537d7e5497dde00a219",
);
#[cfg(not(target_arch = "aarch64"))]
const RUNTIME_ASSET: (&str, &str) = (
    "cpython-3.12.15+20261021-x86_64-apple-darwin-install_only_stripped.tar.gz",
    "f12e0504397683d6be0745ffd4c39ef45a455cec169adf35c628840d06133773",
);

fn env_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("voice-env")
}

fn runtime_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("python")
}

/// The environment's Python, once it has been installed.
pub fn python(data_dir: &Path) -> Option<PathBuf> {
    let python = env_dir(data_dir).join("bin").join("python");
    python.is_file().then_some(python)
}

/// The app's own standalone Python, once downloaded.
fn runtime_python(data_dir: &Path) -> Option<PathBuf> {
    let python = runtime_dir(data_dir).join("bin").join("python3");
    python.is_file().then_some(python)
}

/// Downloads the standalone Python (about 25 MB), checks it and unpacks it.
async fn ensure_runtime(data_dir: &Path) -> AppResult<PathBuf> {
    if let Some(python) = runtime_python(data_dir) {
        return Ok(python);
    }
    let (asset, expected) = RUNTIME_ASSET;
    let curl = shell_env::require_program("curl", "curl comes with macOS; check the PATH.")?;
    let tar = shell_env::require_program("tar", "tar comes with macOS; check the PATH.")?;
    let archive = data_dir.join("python.tar.gz.part");
    let unpack = data_dir.join("python.unpack");
    let clean = || {
        let _ = std::fs::remove_file(&archive);
        let _ = std::fs::remove_dir_all(&unpack);
    };
    clean();

    let mut fetch = shell_env::command(&curl);
    fetch.args(["-fsSL", "--retry", "2", "-o"]).arg(&archive).arg(format!("{RUNTIME_RELEASE}/{asset}"));
    if let Err(error) = step(fetch, RUNTIME_TIMEOUT, "Downloading Python for voiceover").await {
        clean();
        return Err(error);
    }
    let actual = crate::reference::ytdlp::sha256(&archive).await?;
    if actual != expected {
        clean();
        return Err(AppError::ToolFailed(
            "The downloaded Python did not match its published checksum, so it was thrown away.".into(),
        ));
    }
    std::fs::create_dir_all(&unpack)?;
    let mut extract = shell_env::command(&tar);
    extract.arg("-xzf").arg(&archive).arg("-C").arg(&unpack);
    if let Err(error) = step(extract, QUICK_TIMEOUT * 6, "Unpacking Python for voiceover").await {
        clean();
        return Err(error);
    }
    // The archive holds one folder, `python/`.
    let target = runtime_dir(data_dir);
    if target.exists() {
        std::fs::remove_dir_all(&target)?;
    }
    std::fs::rename(unpack.join("python"), &target)?;
    clean();
    runtime_python(data_dir).ok_or_else(|| AppError::ToolFailed("Python for voiceover was not unpacked.".into()))
}

/// Gets the app's Python, makes the environment, installs the packages and
/// speaks one sample so the voice model is downloaded now, not mid-edit.
pub async fn install(data_dir: &Path) -> AppResult<()> {
    let base = ensure_runtime(data_dir).await?;
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
    sample(data_dir, DEFAULT_VOICE).await?;
    Ok(())
}

/// The voice used when none is chosen.
pub const DEFAULT_VOICE: &str = "af_heart";

/// True for a voice id the engine could name: `af_heart`, `bm_george`.
fn plain_voice(voice: &str) -> bool {
    !voice.is_empty() && voice.len() <= 40 && voice.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// A short spoken sample of `voice`, made once and kept. Returns its path.
pub async fn sample(data_dir: &Path, voice: &str) -> AppResult<PathBuf> {
    if !plain_voice(voice) {
        return Err(AppError::InvalidInput("That is not a voice.".into()));
    }
    let folder = data_dir.join("voice-samples");
    let file = folder.join(format!("{voice}.wav"));
    if file.is_file() {
        return Ok(file);
    }
    if python(data_dir).is_none() {
        return Err(AppError::ToolMissing("Voices are not installed yet. Open Check Setup and install them.".into()));
    }
    std::fs::create_dir_all(&folder)?;
    let draft = folder.join(format!("{voice}.part.wav"));
    let target = draft.to_string_lossy().into_owned();
    let text = "Hello! This is how my voice sounds in your video.";
    let output = hyperframes::run(data_dir, &["tts", text, "-v", voice, "-o", &target], &folder, SAMPLE_TIMEOUT).await?;
    if !output.status.success() || !draft.is_file() {
        let _ = std::fs::remove_file(&draft);
        return Err(AppError::ToolFailed(format!("The voice sample could not be made.\n{}", shell_env::failure_text(&output))));
    }
    std::fs::rename(&draft, &file)?;
    Ok(file)
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
    fn only_plain_voice_ids_are_spoken() {
        assert!(plain_voice("af_heart") && plain_voice("bm_george"));
        for bad in ["", "../x", "af heart", "AF_HEART", "a;rm"] {
            assert!(!plain_voice(bad), "{bad}");
        }
    }

    #[tokio::test]
    async fn a_sample_needs_the_voices_installed() {
        let dir = tempfile::tempdir().unwrap();
        let made = sample(dir.path(), "af_heart").await;
        assert_eq!(made.unwrap_err().code(), "tool_missing");
    }

    #[test]
    fn no_environment_means_no_python() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(python(dir.path()), None);
    }
}
