//! HyperFrames' skills for Claude Code: the instructions that tell the
//! assistant how a composition is built, timed, captured and checked.
//!
//! They live in Claude Code's own folder (`~/.claude/skills`), not the
//! app's, because that is where Claude Code looks for them. Without them the
//! assistant would be editing video with no knowledge of the framework, so
//! the app checks for them and can install them with the engine's own
//! installer.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::{AppError, AppResult};
use crate::{hyperframes, shell_env};

/// The skills every video task leans on. Workflow skills (launch videos,
/// explainers and so on) come with the same install.
const CORE: [&str; 10] = [
    "hyperframes",
    "hyperframes-animation",
    "hyperframes-audio",
    "hyperframes-cli",
    "hyperframes-core",
    "hyperframes-creative",
    "hyperframes-keyframes",
    "hyperframes-registry",
    "hyperframes-studio",
    "media-use",
];
const INSTALL_TIMEOUT: Duration = Duration::from_secs(600);

/// Where Claude Code keeps skills for this user.
pub fn folder() -> PathBuf {
    let config = std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).filter(|dir| dir.is_absolute());
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    config.unwrap_or_else(|| home.join(".claude")).join("skills")
}

/// The core skills that are not in `folder`.
pub fn missing(folder: &Path) -> Vec<&'static str> {
    CORE.iter().copied().filter(|name| !folder.join(name).join("SKILL.md").is_file()).collect()
}

pub fn core_count() -> usize {
    CORE.len()
}

/// Refuses with what to do when the skills are not all there.
pub fn require(folder: &Path) -> AppResult<()> {
    if missing(folder).is_empty() {
        Ok(())
    } else {
        Err(AppError::ToolMissing(
            "The assistant needs HyperFrames' video skills. Open Check Setup and choose Install Video Skills.".into(),
        ))
    }
}

/// Runs the engine's installer, which fetches the skills from GitHub and
/// copies them into Claude Code's folder.
pub async fn install(data_dir: &Path) -> AppResult<()> {
    shell_env::require_program("git", "The installer fetches the skills with git. Install the Xcode command line tools (`xcode-select --install`), then try again.")?;
    shell_env::require_program("npx", "Install Node.js 22 or newer, then try again.")?;
    let command = hyperframes::command(data_dir, &["skills"], data_dir)?;
    let output = shell_env::run(command, INSTALL_TIMEOUT, "the video skills install").await?;
    let still_missing = missing(&folder());
    if output.status.success() && still_missing.is_empty() {
        return Ok(());
    }
    Err(AppError::ToolFailed(format!(
        "The video skills could not be installed ({} still missing).\n{}",
        still_missing.len(),
        shell_env::failure_text(&output)
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install_into(folder: &Path, names: &[&str]) {
        for name in names {
            std::fs::create_dir_all(folder.join(name)).unwrap();
            std::fs::write(folder.join(name).join("SKILL.md"), "# skill").unwrap();
        }
    }

    #[test]
    fn an_empty_folder_is_missing_everything() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(missing(dir.path()).len(), CORE.len());
        assert_eq!(missing(&dir.path().join("not-there")).len(), CORE.len());
        assert_eq!(require(dir.path()).unwrap_err().code(), "tool_missing");
    }

    #[test]
    fn a_skill_counts_only_with_its_instructions() {
        let dir = tempfile::tempdir().unwrap();
        install_into(dir.path(), &CORE[..9]);
        std::fs::create_dir_all(dir.path().join(CORE[9])).unwrap(); // a folder with nothing in it
        assert_eq!(missing(dir.path()), [CORE[9]]);
        install_into(dir.path(), &CORE[9..]);
        assert!(missing(dir.path()).is_empty());
        assert!(require(dir.path()).is_ok());
    }

    #[test]
    fn other_skills_do_not_stand_in_for_the_core_ones() {
        let dir = tempfile::tempdir().unwrap();
        install_into(dir.path(), &["product-launch-video", "something-else"]);
        assert_eq!(missing(dir.path()).len(), CORE.len());
    }
}
