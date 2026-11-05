//! The engine's registry of ready-made blocks: transitions, caption styles,
//! textures and more. Listing is cached for a day; installing writes the
//! block into the project and hands back the snippet that places it.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::{hyperframes, shell_env, store};

const LIST_LIMIT: Duration = Duration::from_secs(60);
const ADD_LIMIT: Duration = Duration::from_secs(120);
const FRESH: Duration = Duration::from_secs(24 * 60 * 60);

/// The shelves the app shows, each one registry tag.
pub const SHELVES: [&str; 5] = ["transition", "captions", "texture", "title-card", "overlay"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub name: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    /// Seconds, when the block has a length of its own.
    pub duration: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub name: String,
    /// The markup that places the block, for the assistant to put on the timeline.
    pub snippet: String,
    /// Files written, relative to the project.
    pub files: Vec<String>,
}

fn parse_items(value: &Value) -> Vec<Item> {
    let list = value.as_array().or_else(|| value.get("items").and_then(Value::as_array)).cloned().unwrap_or_default();
    list.iter()
        .filter_map(|item| {
            let text = |key: &str| item.get(key).and_then(Value::as_str).map(str::to_owned);
            Some(Item {
                name: text("name")?,
                title: text("title").unwrap_or_default(),
                description: text("description").unwrap_or_default(),
                tags: item
                    .get("tags")
                    .and_then(Value::as_array)
                    .map(|tags| tags.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                    .unwrap_or_default(),
                duration: item.get("duration").and_then(Value::as_f64),
            })
        })
        .collect()
}

/// The blocks on one shelf.
pub async fn list(data_dir: &Path, shelf: &str) -> AppResult<Vec<Item>> {
    if !SHELVES.contains(&shelf) {
        return Err(AppError::InvalidInput(format!("There is no “{shelf}” shelf.")));
    }
    let cache = data_dir.join("catalog").join(format!("{shelf}-{}.json", hyperframes::PINNED_VERSION));
    let fresh = std::fs::metadata(&cache).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age < FRESH);
    if fresh {
        let cached: Vec<Item> = store::read_or_default(&cache);
        if !cached.is_empty() {
            return Ok(cached);
        }
    }
    let output = hyperframes::run(data_dir, &["catalog", "--json", "--tag", shelf], data_dir, LIST_LIMIT).await?;
    let text = String::from_utf8_lossy(&output.stdout);
    // The answer is a list; notices may come before it, each on a line of its own.
    let value: Value = text
        .lines()
        .position(|line| line.trim_start().starts_with(['[', '{']))
        .and_then(|first| serde_json::from_str(&text.lines().skip(first).collect::<Vec<_>>().join("\n")).ok())
        .ok_or_else(|| AppError::ToolFailed(format!("The catalogue could not be read.\n{}", shell_env::failure_text(&output))))?;
    let items = parse_items(&value);
    store::write_atomic(&cache, &items)?;
    Ok(items)
}

fn check_name(name: &str) -> AppResult<()> {
    let fine = !name.is_empty() && name.len() <= 80 && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if fine {
        Ok(())
    } else {
        Err(AppError::InvalidInput("That is not a catalogue item.".into()))
    }
}

/// Installs a block into the project.
pub async fn add(data_dir: &Path, project: &Path, name: &str) -> AppResult<Installed> {
    check_name(name)?;
    let output = hyperframes::run(data_dir, &["add", name, "--no-clipboard", "--json"], project, ADD_LIMIT).await?;
    let text = String::from_utf8_lossy(&output.stdout);
    let value: Value = text
        .find('{')
        .and_then(|start| serde_json::from_str(&text[start..]).ok())
        .ok_or_else(|| AppError::ToolFailed(format!("“{name}” could not be added.\n{}", shell_env::failure_text(&output))))?;
    if value.get("ok").and_then(Value::as_bool) != Some(true) {
        let reason = value.pointer("/error/message").and_then(Value::as_str).unwrap_or("the engine refused it");
        return Err(AppError::Refused(format!("“{name}” could not be added: {reason}")));
    }
    let files = value
        .get("written")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(|path| Path::new(path).strip_prefix(project).map_or_else(|_| path.to_owned(), |rel| rel.to_string_lossy().into_owned()))
                .collect()
        })
        .unwrap_or_default();
    Ok(Installed {
        name: name.to_owned(),
        snippet: value.get("snippet").and_then(Value::as_str).unwrap_or_default().to_owned(),
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_are_read_from_either_shape() {
        let item = serde_json::json!({"name": "cinematic-zoom", "type": "block", "title": "Cinematic Zoom", "description": "Zoom blur", "tags": ["transition"], "duration": 4});
        assert_eq!(parse_items(&serde_json::json!([item.clone()]))[0].duration, Some(4.0));
        assert_eq!(parse_items(&serde_json::json!({"items": [item, {"title": "no name"}]})).len(), 1);
    }

    #[test]
    fn only_registry_names_are_installed() {
        assert!(check_name("cinematic-zoom").is_ok());
        for bad in ["", "../x", "Zoom", "a b", "a;rm"] {
            assert!(check_name(bad).is_err(), "{bad}");
        }
    }
}
