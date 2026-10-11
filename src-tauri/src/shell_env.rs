//! Finding and running the user's command-line tools.
//!
//! An app started from Finder gets a bare PATH, so tools installed through a
//! version manager (Node under nvm, for one) are invisible to it. The PATH of
//! the user's login shell is read once and used for every child process.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tokio::process::Command;

use crate::error::{AppError, AppResult};

const SHELL_TIMEOUT: Duration = Duration::from_secs(5);
const MARKER: &str = "__RV_PATH__";

static SEARCH_PATH: OnceLock<OsString> = OnceLock::new();

/// The directories searched for tools: the login shell's PATH, then this
/// process's own, then the places tools usually install themselves.
pub fn search_path() -> &'static OsString {
    SEARCH_PATH.get_or_init(|| {
        let mut dirs: Vec<PathBuf> = Vec::new();
        if let Some(path) = login_shell_path() {
            dirs.extend(std::env::split_paths(&path));
        }
        if let Some(path) = std::env::var_os("PATH") {
            dirs.extend(std::env::split_paths(&path));
        }
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            dirs.push(home.join(".local/bin"));
            dirs.push(home.join(".claude/local"));
        }
        dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"].map(PathBuf::from));

        let mut seen = std::collections::HashSet::new();
        dirs.retain(|dir| seen.insert(dir.clone()));
        std::env::join_paths(dirs).unwrap_or_default()
    })
}

/// Asks the user's shell for its PATH. The markers keep anything the shell's
/// start-up files print out of the answer.
fn login_shell_path() -> Option<OsString> {
    let shell = std::env::var_os("SHELL").unwrap_or_else(|| "/bin/zsh".into());
    let script = format!("printf '{MARKER}%s{MARKER}' \"$PATH\"");
    let mut child = std::process::Command::new(shell)
        .args(["-ilc", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < SHELL_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(25));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let output = child.wait_with_output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let path = text.split(MARKER).nth(1)?.trim();
    (!path.is_empty()).then(|| OsString::from(path))
}

pub fn find_program(name: &str) -> Option<PathBuf> {
    std::env::split_paths(search_path())
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

pub fn require_program(name: &str, hint: &str) -> AppResult<PathBuf> {
    find_program(name).ok_or_else(|| AppError::ToolMissing(format!("{name} was not found. {hint}")))
}

/// A child process that sees the user's tools and dies with the app.
pub fn command(program: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .env("PATH", search_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    command
}

/// Runs to completion or gives up after `limit`. `what` names the step in the
/// error the user reads.
pub async fn run(mut command: Command, limit: Duration, what: &str) -> AppResult<Output> {
    let child = command
        .spawn()
        .map_err(|error| AppError::ToolFailed(format!("Could not start {what}: {error}")))?;
    match tokio::time::timeout(limit, child.wait_with_output()).await {
        Ok(output) => Ok(output?),
        Err(_) => Err(AppError::Timeout(format!(
            "{what} did not finish within {} seconds.",
            limit.as_secs()
        ))),
    }
}

/// The last lines a failed tool printed, which is where it says why.
pub fn failure_text(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = if stderr.trim().is_empty() { stdout } else { stderr };
    let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
    let tail = lines[lines.len().saturating_sub(6)..].join("\n");
    if tail.is_empty() {
        format!("exit status {}", output.status)
    } else {
        tail
    }
}

/// The first line of a message, for places that show one line.
pub fn first_line(text: &str) -> String {
    text.lines().find(|line| !line.trim().is_empty()).unwrap_or("").trim().to_owned()
}

