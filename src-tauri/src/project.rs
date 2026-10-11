//! Projects: a project is an ordinary HyperFrames folder anywhere on disk.
//! The app opens it in place and never copies it.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::{hyperframes, shell_env};

const COMPOSITION_MARKER: &str = "data-composition-id";
/// How much of index.html is read when looking for the marker.
const MARKER_SCAN_BYTES: u64 = 512 * 1024;
const MAX_NAME_LENGTH: usize = 64;
const INFO_TIMEOUT: Duration = Duration::from_secs(30);
const CREATE_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ProjectSummary {
    pub path: String,
    pub name: String,
}

/// What the engine reports about a project's composition.
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub elements: u32,
    pub tracks: u32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Resolution {
    Landscape,
    Portrait,
    Square,
}

impl Resolution {
    fn flag(self) -> &'static str {
        match self {
            Self::Landscape => "--resolution=landscape",
            Self::Portrait => "--resolution=portrait",
            Self::Square => "--resolution=square",
        }
    }
}

/// Checks that `path` is a HyperFrames project and says what it is called.
pub fn inspect(path: &Path) -> AppResult<ProjectSummary> {
    if !path.is_absolute() {
        return Err(AppError::InvalidInput("A project path must be absolute.".into()));
    }
    let dir = path
        .canonicalize()
        .map_err(|_| AppError::NotFound(format!("{} no longer exists.", path.display())))?;
    if !dir.is_dir() {
        return Err(AppError::NotAProject(format!("{} is not a folder.", dir.display())));
    }
    let index = dir.join("index.html");
    if !index.is_file() || !has_composition(&index)? {
        return Err(AppError::NotAProject(format!(
            "{} is not a HyperFrames project: it has no index.html with a composition.",
            dir.display()
        )));
    }
    Ok(ProjectSummary { name: project_name(&dir), path: dir.to_string_lossy().into_owned() })
}

fn has_composition(index: &Path) -> AppResult<bool> {
    let mut head = Vec::new();
    std::fs::File::open(index)?.take(MARKER_SCAN_BYTES).read_to_end(&mut head)?;
    Ok(String::from_utf8_lossy(&head).contains(COMPOSITION_MARKER))
}

/// The name in meta.json when there is one, the folder's name otherwise.
fn project_name(dir: &Path) -> String {
    let from_meta = std::fs::read_to_string(dir.join("meta.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|meta| meta.get("name")?.as_str().map(str::trim).map(str::to_owned))
        .filter(|name| !name.is_empty());
    from_meta.unwrap_or_else(|| {
        dir.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
    })
}

/// Turns what the user typed into a folder name: lower case, digits and
/// single hyphens.
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for ch in name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_end_matches('-').to_owned()
}

/// Where a new project would go, or why it cannot be made there.
pub fn plan_new(parent: &Path, name: &str) -> AppResult<(PathBuf, String)> {
    let slug = slugify(name);
    if slug.is_empty() {
        return Err(AppError::InvalidInput("Give the project a name with letters or numbers.".into()));
    }
    if slug.len() > MAX_NAME_LENGTH {
        return Err(AppError::InvalidInput(format!(
            "A project name can be at most {MAX_NAME_LENGTH} characters."
        )));
    }
    if !parent.is_absolute() || !parent.is_dir() {
        return Err(AppError::NotFound(format!("The folder {} does not exist.", parent.display())));
    }
    let target = parent.join(&slug);
    if target.exists() {
        return Err(AppError::AlreadyExists(format!(
            "{} already exists. Choose another name or location.",
            target.display()
        )));
    }
    Ok((target, slug))
}

/// Scaffolds a blank project named `name` inside `parent`.
pub async fn create(
    data_dir: &Path,
    parent: &Path,
    name: &str,
    resolution: Resolution,
) -> AppResult<ProjectSummary> {
    let (target, slug) = plan_new(parent, name)?;
    let args = [
        "init",
        slug.as_str(),
        "--non-interactive",
        "--example=blank",
        "--skill=general-video",
        resolution.flag(),
    ];
    let output = hyperframes::run(data_dir, &args, parent, CREATE_TIMEOUT).await?;
    if !output.status.success() {
        return Err(AppError::ToolFailed(format!(
            "The project could not be created.\n{}",
            shell_env::failure_text(&output)
        )));
    }
    inspect(&target)
}

pub async fn info(data_dir: &Path, path: &Path) -> AppResult<ProjectInfo> {
    let summary = inspect(path)?;
    hyperframes::run_json(data_dir, &["info", "--json"], Path::new(&summary.path), INFO_TIMEOUT).await
}

/// Selects the project folder in Finder.
pub async fn reveal(path: &Path) -> AppResult<()> {
    let summary = inspect(path)?;
    reveal_file(Path::new(&summary.path)).await
}

/// Selects a file or folder in Finder. The caller has checked the path.
pub async fn reveal_file(path: &Path) -> AppResult<()> {
    let mut command = shell_env::command(Path::new("/usr/bin/open"));
    command.arg("-R").arg(path);
    let output = shell_env::run(command, Duration::from_secs(10), "Finder").await?;
    if output.status.success() {
        Ok(())
    } else {
        Err(AppError::ToolFailed(shell_env::failure_text(&output)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_in(dir: &Path, index: &str) {
        std::fs::write(dir.join("index.html"), index).unwrap();
    }

    #[test]
    fn a_folder_with_a_composition_is_a_project() {
        let dir = tempfile::tempdir().unwrap();
        project_in(dir.path(), r#"<div data-composition-id="main"></div>"#);
        std::fs::write(dir.path().join("meta.json"), r#"{"name":"Launch film"}"#).unwrap();
        let summary = inspect(dir.path()).unwrap();
        assert_eq!(summary.name, "Launch film");
        assert_eq!(Path::new(&summary.path), dir.path().canonicalize().unwrap());
    }

    #[test]
    fn the_folder_name_is_used_without_meta() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("promo");
        std::fs::create_dir(&project).unwrap();
        project_in(&project, r#"<div data-composition-id="main"></div>"#);
        assert_eq!(inspect(&project).unwrap().name, "promo");
    }

    #[test]
    fn a_plain_web_page_is_not_a_project() {
        let dir = tempfile::tempdir().unwrap();
        project_in(dir.path(), "<h1>hello</h1>");
        assert_eq!(inspect(dir.path()).unwrap_err().code(), "not_a_project");
    }

    #[test]
    fn an_empty_folder_is_not_a_project() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(inspect(dir.path()).unwrap_err().code(), "not_a_project");
    }

    #[test]
    fn a_missing_folder_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(inspect(&dir.path().join("gone")).unwrap_err().code(), "not_found");
    }

    #[test]
    fn a_relative_path_is_refused() {
        assert_eq!(inspect(Path::new("some/where")).unwrap_err().code(), "invalid_input");
    }

    #[test]
    fn names_become_folder_slugs() {
        assert_eq!(slugify("  ERPTick — Launch Film! "), "erptick-launch-film");
        assert_eq!(slugify("a__b"), "a-b");
        assert_eq!(slugify("../../etc"), "etc");
        assert_eq!(slugify("***"), "");
    }

    #[test]
    fn a_new_project_needs_a_usable_name_and_a_free_folder() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(plan_new(dir.path(), "!!!").unwrap_err().code(), "invalid_input");
        assert_eq!(plan_new(dir.path(), &"a".repeat(65)).unwrap_err().code(), "invalid_input");
        assert_eq!(plan_new(&dir.path().join("gone"), "film").unwrap_err().code(), "not_found");

        std::fs::create_dir(dir.path().join("film")).unwrap();
        assert_eq!(plan_new(dir.path(), "Film").unwrap_err().code(), "already_exists");

        let (target, slug) = plan_new(dir.path(), "New Film").unwrap();
        assert_eq!(slug, "new-film");
        assert_eq!(target, dir.path().join("new-film"));
    }
}
