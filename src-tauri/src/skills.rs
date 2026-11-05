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

/// A skill the assistant can be pointed at from the chat.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
}

/// The skill's `name` and the start of its `description`, from SKILL.md's front matter.
fn read_front_matter(text: &str) -> Option<(String, String)> {
    let body = text.strip_prefix("---")?;
    let end = body.find("\n---")?;
    let mut name = None;
    let mut description = String::new();
    for line in body[..end].lines() {
        if let Some(value) = line.strip_prefix("name:") {
            name = Some(value.trim().trim_matches('"').to_owned());
        } else if let Some(value) = line.strip_prefix("description:") {
            description = value.trim().trim_matches(['"', '\'', '>', '|']).trim().to_owned();
        }
    }
    let description: String = description.chars().take(160).collect();
    Some((name?, description))
}

/// The installed skills about video: HyperFrames' own and any that mention it.
pub fn list(folder: &Path) -> Vec<SkillInfo> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut found: Vec<SkillInfo> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| read_front_matter(&std::fs::read_to_string(entry.path().join("SKILL.md")).ok()?))
        .filter(|(name, description)| {
            CORE.contains(&name.as_str()) || format!("{name} {description}").to_lowercase().contains("hyperframes")
        })
        .map(|(name, description)| SkillInfo { name, description })
        .collect();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

#[cfg(test)]
mod list_tests {
    use super::*;

    #[test]
    fn only_video_skills_are_listed_with_their_description() {
        let dir = tempfile::tempdir().unwrap();
        let write = |folder: &str, text: &str| {
            std::fs::create_dir_all(dir.path().join(folder)).unwrap();
            std::fs::write(dir.path().join(folder).join("SKILL.md"), text).unwrap();
        };
        write("captions", "---\nname: embedded-captions\ndescription: \"Add captions to a HyperFrames video\"\n---\nbody");
        write("pdf", "---\nname: pdf\ndescription: Read PDFs\n---\n");
        write("broken", "no front matter");
        let found = list(dir.path());
        assert_eq!(found, vec![SkillInfo { name: "embedded-captions".into(), description: "Add captions to a HyperFrames video".into() }]);
    }
}
