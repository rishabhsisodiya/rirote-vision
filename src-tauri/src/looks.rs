//! Looks: the engine's tested colour presets, applied to a video or image clip
//! with `media-treatment`. Exports render them; the preview shows them.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::{hyperframes, shell_env, store};

const LIMIT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Look {
    pub id: String,
    pub label: String,
}

fn parse_presets(value: &Value) -> Vec<Look> {
    value
        .pointer("/capabilities/presets")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|preset| {
                    Some(Look {
                        id: preset.get("id")?.as_str()?.to_owned(),
                        label: preset.get("label").and_then(Value::as_str).unwrap_or_default().to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The presets this engine offers; read once per engine version.
pub async fn list(data_dir: &Path) -> AppResult<Vec<Look>> {
    let cache = data_dir.join("catalog").join(format!("looks-{}.json", hyperframes::PINNED_VERSION));
    let cached: Vec<Look> = store::read_or_default(&cache);
    if !cached.is_empty() {
        return Ok(cached);
    }
    let value: Value =
        hyperframes::run_json(data_dir, &["media-treatment", "--capability", "presets"], data_dir, LIMIT).await?;
    let looks = parse_presets(&value);
    store::write_atomic(&cache, &looks)?;
    Ok(looks)
}

fn selector(clip_id: &str) -> AppResult<String> {
    let fine = !clip_id.is_empty() && clip_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if fine {
        Ok(format!("#{clip_id}"))
    } else {
        Err(AppError::InvalidInput("A look can only go on a clip with a plain id.".into()))
    }
}

fn check_preset(preset: &str) -> AppResult<()> {
    if !preset.is_empty() && preset.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        Ok(())
    } else {
        Err(AppError::InvalidInput("That is not a look.".into()))
    }
}

/// Puts a look on a clip, or takes it off when `preset` is None.
pub async fn apply(data_dir: &Path, project: &Path, clip_id: &str, preset: Option<&str>, intensity: f64) -> AppResult<()> {
    let target = selector(clip_id)?;
    let mut args: Vec<String> = vec!["media-treatment".into(), "--selector".into(), target, "--json".into()];
    match preset {
        Some(preset) => {
            check_preset(preset)?;
            if !(0.0..=1.0).contains(&intensity) {
                return Err(AppError::InvalidInput("Strength goes from 0 to 100%.".into()));
            }
            let grading = serde_json::json!({ "preset": preset, "intensity": (intensity * 100.0).round() / 100.0 });
            args.extend(["--grading".into(), grading.to_string(), "--apply".into()]);
        }
        None => args.push("--clear".into()),
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = hyperframes::run(data_dir, &refs, project, LIMIT).await?;
    let text = String::from_utf8_lossy(&output.stdout);
    let answer: Value = text.find('{').and_then(|start| serde_json::from_str(&text[start..]).ok()).unwrap_or(Value::Null);
    if answer.get("ok").and_then(Value::as_bool) == Some(true) {
        return Ok(());
    }
    let reason = answer
        .pointer("/error/message")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| shell_env::failure_text(&output));
    Err(AppError::Refused(format!("The look could not be changed: {reason}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_read_and_names_checked() {
        let value = serde_json::json!({"ok": true, "capabilities": {"presets": [{"id": "warm-daylight", "label": "Warm Daylight"}, {"label": "no id"}]}});
        assert_eq!(parse_presets(&value), vec![Look { id: "warm-daylight".into(), label: "Warm Daylight".into() }]);
        assert!(check_preset("vhs-playback").is_ok());
        assert!(check_preset("Bad; rm").is_err());
        assert_eq!(selector("clip-1").unwrap(), "#clip-1");
        assert!(selector("a b").is_err() && selector("").is_err());
    }
}
