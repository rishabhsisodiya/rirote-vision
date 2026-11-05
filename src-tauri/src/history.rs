//! The engine's project history: every assistant turn is recorded as one
//! entry, which can be listed and undone as a whole.

use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::{hyperframes, log};

const LIMIT: Duration = Duration::from_secs(30);
/// The name the assistant's turns are recorded under.
pub const ASSISTANT: &str = "rirote-assistant";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub label: String,
    /// "agent" for the assistant, "person" for edits made by hand.
    pub by: String,
    pub name: String,
    /// Milliseconds since 1970.
    pub ended_at: Option<f64>,
    pub files: Vec<String>,
    pub undone: bool,
}

fn label_of(prompt: &str) -> String {
    let line = prompt.lines().find(|line| !line.trim().is_empty()).unwrap_or("Assistant turn").trim();
    let mut label: String = line.chars().take(72).collect();
    if line.chars().count() > 72 {
        label.push('…');
    }
    label
}

async fn run(data_dir: &Path, project: &Path, args: &[&str]) -> AppResult<Value> {
    hyperframes::run_json(data_dir, args, project, LIMIT).await
}

pub async fn begin(data_dir: &Path, project: &Path, prompt: &str) {
    let label = label_of(prompt);
    if let Err(error) = run(data_dir, project, &["history", "begin", "--json", "--who", ASSISTANT, "--label", &label]).await {
        log::write(data_dir, "history", &format!("turn not recorded: {error}"));
    }
}

pub async fn end(data_dir: &Path, project: &Path) {
    if let Err(error) = run(data_dir, project, &["history", "end", "--json"]).await {
        log::write(data_dir, "history", &format!("turn not closed: {error}"));
    }
}

fn parse_entry(value: &Value) -> Option<Entry> {
    let text = |pointer: &str| value.pointer(pointer).and_then(Value::as_str).map(str::to_owned);
    Some(Entry {
        id: text("/id")?,
        label: text("/label").unwrap_or_default(),
        by: text("/who/kind").unwrap_or_default(),
        name: text("/who/name").unwrap_or_default(),
        ended_at: value.get("endedAt").and_then(Value::as_f64),
        files: value
            .get("files")
            .and_then(Value::as_array)
            .map(|files| files.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default(),
        undone: value.get("undone").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// The newest entries first.
pub async fn list(data_dir: &Path, project: &Path) -> AppResult<Vec<Entry>> {
    let value = run(data_dir, project, &["history", "--json", "--limit", "30"]).await?;
    Ok(value.get("entries").and_then(Value::as_array).map(|list| list.iter().filter_map(parse_entry).collect()).unwrap_or_default())
}

pub async fn undo(data_dir: &Path, project: &Path, id: &str) -> AppResult<()> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return Err(AppError::InvalidInput("That is not a history entry.".into()));
    }
    let answer = run(data_dir, project, &["history", "undo", id, "--json"]).await?;
    if answer.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(())
    } else {
        let reason = answer.pointer("/error/message").and_then(Value::as_str).unwrap_or("the engine refused");
        Err(AppError::Refused(format!("That turn could not be undone: {reason}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_the_first_line_kept_short() {
        assert_eq!(label_of("\n  Make scene 2 punchier\nmore"), "Make scene 2 punchier");
        assert_eq!(label_of(&"a".repeat(100)).chars().count(), 73);
        assert_eq!(label_of(""), "Assistant turn");
    }

    #[test]
    fn entries_are_read_from_the_engine() {
        let value = serde_json::json!({"id": "6c7a-1", "who": {"kind": "agent", "name": "rirote-assistant"}, "label": "Trim", "endedAt": 1.5, "files": ["index.html"], "undone": false});
        let entry = parse_entry(&value).unwrap();
        assert_eq!((entry.by.as_str(), entry.files.len(), entry.undone), ("agent", 1, false));
        assert!(parse_entry(&serde_json::json!({"label": "no id"})).is_none());
    }
}
