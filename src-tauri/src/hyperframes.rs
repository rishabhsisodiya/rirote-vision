//! The video engine: a HyperFrames CLI pinned to one version and kept in the
//! app's own folder, so a project renders the same way every time and nothing
//! depends on what `npx` happens to have cached.

use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::process::Command;

use crate::error::{AppError, AppResult};
use crate::{shell_env, voice};

pub const PINNED_VERSION: &str = "0.9.9";

const INSTALL_TIMEOUT: Duration = Duration::from_secs(600);
const NODE_HINT: &str = "Install Node.js 22 or newer, then check the setup again.";

fn toolchain_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("toolchain")
}

fn package_dir(data_dir: &Path) -> PathBuf {
    toolchain_dir(data_dir).join("node_modules").join("hyperframes")
}

/// The script that runs the engine.
pub fn entry(data_dir: &Path) -> PathBuf {
    package_dir(data_dir).join("bin").join("hyperframes.mjs")
}

/// The engine version in the app's folder, if one is there.
pub fn installed_version(data_dir: &Path) -> Option<String> {
    let manifest = std::fs::read_to_string(package_dir(data_dir).join("package.json")).ok()?;
    let manifest: Value = serde_json::from_str(&manifest).ok()?;
    manifest.get("version")?.as_str().map(str::to_owned)
}

pub fn is_ready(data_dir: &Path) -> bool {
    installed_version(data_dir).as_deref() == Some(PINNED_VERSION)
}

/// Downloads the pinned engine from npm into the app's folder.
pub async fn install(data_dir: &Path) -> AppResult<()> {
    let npm = shell_env::require_program("npm", NODE_HINT)?;
    let dir = toolchain_dir(data_dir);
    std::fs::create_dir_all(&dir)?;
    let manifest = dir.join("package.json");
    if !manifest.exists() {
        std::fs::write(&manifest, "{\n  \"name\": \"rirote-vision-toolchain\",\n  \"private\": true\n}\n")?;
    }

    let mut command = shell_env::command(&npm);
    command
        .current_dir(&dir)
        .args(["install", "--save-exact", "--no-audit", "--no-fund", "--loglevel=error"])
        .arg(format!("hyperframes@{PINNED_VERSION}"));
    let output = shell_env::run(command, INSTALL_TIMEOUT, "the video engine download").await?;
    if !output.status.success() {
        return Err(AppError::ToolFailed(format!(
            "The video engine could not be installed.\n{}",
            shell_env::failure_text(&output)
        )));
    }
    if !is_ready(data_dir) {
        return Err(AppError::ToolFailed(
            "The video engine download finished but the engine is not in place.".into(),
        ));
    }
    Ok(())
}

/// An engine command ready to run in `cwd`. The engine's usage reporting and
/// update checks are switched off: this app pins the version itself.
pub fn command(data_dir: &Path, args: &[&str], cwd: &Path) -> AppResult<Command> {
    if !is_ready(data_dir) {
        return Err(AppError::ToolMissing(
            "The video engine is not installed yet. Install it from Check Setup.".into(),
        ));
    }
    let node = shell_env::require_program("node", NODE_HINT)?;
    let mut command = shell_env::command(&node);
    command
        .arg(entry(data_dir))
        .args(args)
        .current_dir(cwd)
        // The engine names a project after $PWD, which a child process
        // inherits from the app unless it is set to match.
        .env("PWD", cwd)
        .env_remove("INIT_CWD")
        .env("HYPERFRAMES_NO_TELEMETRY", "1")
        .env("HYPERFRAMES_NO_UPDATE_CHECK", "1");
    // Text-to-speech runs in the app's own Python environment when it has one.
    if let Some(python) = voice::python(data_dir) {
        command.env("HYPERFRAMES_PYTHON", python);
    }
    Ok(command)
}

/// Runs one engine command in `cwd` to completion.
pub async fn run(data_dir: &Path, args: &[&str], cwd: &Path, limit: Duration) -> AppResult<Output> {
    let what = format!("hyperframes {}", args.first().copied().unwrap_or_default());
    shell_env::run(command(data_dir, args, cwd)?, limit, &what).await
}

/// Runs an engine command that answers in JSON.
pub async fn run_json<T: DeserializeOwned>(
    data_dir: &Path,
    args: &[&str],
    cwd: &Path,
    limit: Duration,
) -> AppResult<T> {
    let output = run(data_dir, args, cwd, limit).await?;
    parse_json(&output.stdout).ok_or_else(|| {
        AppError::ToolFailed(format!(
            "hyperframes {} gave an answer the app could not read.\n{}",
            args.first().copied().unwrap_or_default(),
            shell_env::failure_text(&output)
        ))
    })
}

/// The engine may print notices before its JSON, so parsing starts at the
/// first brace.
fn parse_json<T: DeserializeOwned>(stdout: &[u8]) -> Option<T> {
    let text = String::from_utf8_lossy(stdout);
    let start = text.find('{')?;
    serde_json::from_str(&text[start..]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_json_after_a_notice() {
        let value: Value = parse_json(b"update available\n{\"ok\":true}\n").unwrap();
        assert_eq!(value["ok"], true);
    }

    #[test]
    fn no_json_is_none() {
        assert!(parse_json::<Value>(b"nothing here").is_none());
    }

    #[test]
    fn missing_engine_is_not_ready() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(installed_version(dir.path()), None);
        assert!(!is_ready(dir.path()));
    }

    #[test]
    fn only_the_pinned_version_is_ready() {
        let dir = tempfile::tempdir().unwrap();
        let package = package_dir(dir.path());
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join("package.json"), r#"{"version":"0.0.1"}"#).unwrap();
        assert!(!is_ready(dir.path()));
        let pinned = format!(r#"{{"version":"{PINNED_VERSION}"}}"#);
        std::fs::write(package.join("package.json"), pinned).unwrap();
        assert!(is_ready(dir.path()));
    }
}
