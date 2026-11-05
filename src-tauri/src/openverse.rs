//! Free sound effects and music from Openverse (openverse.org), which needs
//! no key. Each download is saved into the project with its credit line in
//! `assets/sounds/CREDITS.txt`, as the licences ask.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::{media, shell_env};

const API: &str = "https://api.openverse.org/v1/audio/";
const SEARCH_LIMIT: Duration = Duration::from_secs(30);
const DOWNLOAD_LIMIT: Duration = Duration::from_secs(120);
const MAX_BYTES: &str = "40000000";
/// The most results Openverse gives without a key.
const PAGE_SIZE: u32 = 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sound {
    pub id: String,
    pub title: String,
    pub creator: Option<String>,
    /// cc0, by, by-sa, by-nc and so on.
    pub license: String,
    /// Seconds, when known.
    pub duration: Option<f64>,
    /// The file to download.
    pub url: String,
    /// Where the sound is published, for the credit.
    pub page: Option<String>,
    /// The credit line the licence asks for.
    pub attribution: String,
    pub filetype: Option<String>,
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (byte as char).to_string(),
            b' ' => "+".into(),
            other => format!("%{other:02X}"),
        })
        .collect()
}

fn parse(value: &Value) -> Vec<Sound> {
    value
        .get("results")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|item| {
                    let text = |key: &str| item.get(key).and_then(Value::as_str).map(str::to_owned);
                    let url = text("url").filter(|url| url.starts_with("https://"))?;
                    Some(Sound {
                        id: text("id")?,
                        title: text("title").unwrap_or_else(|| "Untitled".into()),
                        creator: text("creator"),
                        license: text("license").unwrap_or_default(),
                        // Openverse gives milliseconds.
                        duration: item.get("duration").and_then(Value::as_f64).map(|ms| ms / 1000.0),
                        url,
                        page: text("foreign_landing_url"),
                        attribution: text("attribution").unwrap_or_default(),
                        filetype: text("filetype"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Sounds matching `query`; with `commercial`, only licences that allow commercial use.
pub async fn search(query: &str, commercial: bool) -> AppResult<Vec<Sound>> {
    let query = query.trim();
    if query.is_empty() || query.len() > 100 {
        return Err(AppError::InvalidInput("Search for one to a few words, like “whoosh” or “calm piano”.".into()));
    }
    let curl = shell_env::require_program("curl", "curl comes with macOS; check the PATH.")?;
    let mut url = format!("{API}?q={}&page_size={PAGE_SIZE}", encode(query));
    if commercial {
        url.push_str("&license_type=commercial");
    }
    let mut command = shell_env::command(&curl);
    // No -f: a refusal's body says why, and is worth showing.
    command.args(["-sSL", "-H", "Accept: application/json", &url]);
    let output = shell_env::run(command, SEARCH_LIMIT, "the sound search").await?;
    let value: Value = match serde_json::from_slice(&output.stdout) {
        Ok(value) if output.status.success() => value,
        _ => {
            return Err(AppError::Refused("Openverse did not answer. Check the internet connection, or try again in a minute.".into()));
        }
    };
    if value.get("results").is_none() {
        let detail = value.get("detail").and_then(Value::as_str).unwrap_or("no reason given");
        return Err(AppError::Refused(format!("Openverse refused the search: {detail}")));
    }
    Ok(parse(&value))
}

fn file_name(sound: &Sound) -> String {
    let mut stem: String = sound
        .title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    stem.truncate(40);
    if stem.is_empty() {
        stem = "sound".into();
    }
    let extension = sound
        .filetype
        .as_deref()
        .filter(|ext| ["mp3", "wav", "ogg", "flac", "m4a", "aac"].contains(ext))
        .unwrap_or("mp3");
    format!("{stem}.{extension}")
}

/// Downloads the sound into `assets/sounds/` and records its credit. Returns
/// the file's path inside the project.
pub async fn download(project: &Path, sound: &Sound) -> AppResult<String> {
    if !sound.url.starts_with("https://") {
        return Err(AppError::InvalidInput("That sound cannot be downloaded safely.".into()));
    }
    let folder = media::assets_dir(project).join("sounds");
    std::fs::create_dir_all(&folder)?;
    let target: PathBuf = media::free_name(&folder, Path::new(&file_name(sound)));
    let draft = target.with_extension("part");
    let curl = shell_env::require_program("curl", "curl comes with macOS; check the PATH.")?;
    let mut command = shell_env::command(&curl);
    command.args(["-fsSL", "--max-filesize", MAX_BYTES, "-o"]).arg(&draft).arg(&sound.url);
    let output = shell_env::run(command, DOWNLOAD_LIMIT, "the sound download").await?;
    if !output.status.success() || !draft.is_file() {
        let _ = std::fs::remove_file(&draft);
        return Err(AppError::Refused("The sound could not be downloaded. Its host may be down; try another one.".into()));
    }
    std::fs::rename(&draft, &target)?;
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut credits = std::fs::OpenOptions::new().create(true).append(true).open(folder.join("CREDITS.txt"))?;
    writeln!(credits, "{name}: {}{}", sound.attribution.trim(), sound.page.as_deref().map(|page| format!(" ({page})")).unwrap_or_default())?;
    Ok(target.strip_prefix(project).unwrap_or(&target).to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_are_read_and_only_https_files_kept() {
        let value = serde_json::json!({"results": [
            {"id": "a", "title": "Deep Whoosh #1", "creator": "Kinoton", "license": "cc0", "url": "https://cdn.example/a.mp3", "duration": 3155, "attribution": "credit", "foreign_landing_url": "https://freesound.org/x", "filetype": "mp3"},
            {"id": "b", "title": "Plain", "url": "http://insecure/b.mp3"}
        ]});
        let found = parse(&value);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].duration, Some(3.155));
        assert_eq!(file_name(&found[0]), "deep-whoosh-1.mp3");
    }

    #[test]
    fn queries_are_encoded() {
        assert_eq!(encode("calm piano & rain"), "calm+piano+%26+rain");
    }
}
