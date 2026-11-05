//! A media file's words, as the transcription job left them beside it, and
//! its silences, for editing by text and shortening pauses.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::{media, shell_env};

const SILENCE_LIMIT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Word {
    pub text: String,
    /// Seconds into the media file.
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    /// The transcript file, relative to the project.
    pub file: String,
    pub words: Vec<Word>,
}

/// The transcript the transcription job wrote for `source`: `<name>.transcript.json`
/// beside it, or the newest numbered one (`<name>-2.transcript.json`).
fn transcript_file(source: &Path) -> Option<PathBuf> {
    let folder = source.parent()?;
    let stem = source.file_stem()?.to_string_lossy().into_owned();
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(folder)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let base = name.strip_suffix(".transcript.json")?;
            let ours = base == stem
                || base
                    .strip_prefix(&format!("{stem}-"))
                    .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
            ours.then(|| (entry.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), entry.path()))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().next().map(|(_, path)| path)
}

fn parse_words(value: &Value) -> Vec<Word> {
    // The engine writes a list of words; some versions wrap it in `words`.
    let list = value.as_array().or_else(|| value.get("words").and_then(Value::as_array)).cloned().unwrap_or_default();
    list.iter()
        .filter_map(|word| {
            let text = word.get("text").or_else(|| word.get("word")).and_then(Value::as_str)?.trim().to_owned();
            let start = word.get("start").and_then(Value::as_f64)?;
            let end = word.get("end").and_then(Value::as_f64)?;
            (!text.is_empty() && end >= start).then_some(Word { text, start, end })
        })
        .collect()
}

pub fn read(project: &Path, src: &str) -> AppResult<Option<Transcript>> {
    let source = media::resolve(project, src)?;
    let Some(file) = transcript_file(&source) else { return Ok(None) };
    let value: Value = serde_json::from_str(&std::fs::read_to_string(&file)?)
        .map_err(|_| AppError::Io(format!("{} is not a transcript the app can read.", file.display())))?;
    Ok(Some(Transcript {
        file: file.strip_prefix(project).unwrap_or(&file).to_string_lossy().into_owned(),
        words: parse_words(&value),
    }))
}

/// Stretches quieter than `noise_db` for at least `min_seconds`, in file seconds.
pub async fn silences(project: &Path, src: &str, noise_db: f64, min_seconds: f64) -> AppResult<Vec<(f64, f64)>> {
    if !(-80.0..=-10.0).contains(&noise_db) || !(0.1..=10.0).contains(&min_seconds) {
        return Err(AppError::InvalidInput("Quiet means −80 to −10 dB, for 0.1 to 10 seconds.".into()));
    }
    let source = media::resolve(project, src)?;
    let ffmpeg = shell_env::require_program("ffmpeg", "Install FFmpeg with `brew install ffmpeg`.")?;
    let mut command = shell_env::command(&ffmpeg);
    command
        .args(["-hide_banner", "-nostats", "-i"])
        .arg(&source)
        .args(["-vn", "-af", &format!("silencedetect=n={noise_db}dB:d={min_seconds}"), "-f", "null", "-"]);
    let output = shell_env::run(command, SILENCE_LIMIT, "ffmpeg").await?;
    if !output.status.success() {
        return Err(AppError::ToolFailed(format!("The pauses could not be found.\n{}", shell_env::failure_text(&output))));
    }
    Ok(parse_silences(&String::from_utf8_lossy(&output.stderr)))
}

fn parse_silences(log: &str) -> Vec<(f64, f64)> {
    let mut found = Vec::new();
    let mut open: Option<f64> = None;
    for line in log.lines() {
        let number = |key: &str| line.split(key).nth(1)?.split_whitespace().next()?.parse::<f64>().ok();
        if let Some(start) = number("silence_start:") {
            open = Some(start.max(0.0));
        } else if let (Some(end), Some(start)) = (number("silence_end:"), open) {
            found.push((start, end));
            open = None;
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_transcript_of_the_file_is_found_and_no_other() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("talk.mp4");
        std::fs::write(&source, "").unwrap();
        assert_eq!(transcript_file(&source), None);
        std::fs::write(dir.path().join("talk-other.transcript.json"), "[]").unwrap();
        assert_eq!(transcript_file(&source), None, "another file's transcript is not ours");
        std::fs::write(dir.path().join("talk.transcript.json"), "[]").unwrap();
        assert_eq!(transcript_file(&source).unwrap().file_name().unwrap(), "talk.transcript.json");
    }

    #[test]
    fn words_come_from_either_shape() {
        let list = serde_json::json!([{"text": "Hi", "start": 0.1, "end": 0.3, "id": "w0"}, {"text": " ", "start": 0.3, "end": 0.4}]);
        assert_eq!(parse_words(&list), vec![Word { text: "Hi".into(), start: 0.1, end: 0.3 }]);
        let wrapped = serde_json::json!({"words": [{"word": "Yo", "start": 1.0, "end": 1.2}]});
        assert_eq!(parse_words(&wrapped)[0].text, "Yo");
    }

    #[test]
    fn silences_are_read_from_ffmpegs_log() {
        let log = "[silencedetect @ 0x1] silence_start: 1.25\n[silencedetect @ 0x1] silence_end: 2.5 | silence_duration: 1.25\n[x] silence_start: -0.01\n[x] silence_end: 0.6 | silence_duration: 0.6\n[x] silence_start: 9";
        assert_eq!(parse_silences(log), vec![(1.25, 2.5), (0.0, 0.6)]);
    }
}
