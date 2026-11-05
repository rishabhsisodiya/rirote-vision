//! What the timeline draws inside a clip: a sound file's waveform and a
//! video's strip of stills. Each is made once per file version and kept in
//! the app's folder.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::{ids, media, shell_env, store};

const LIMIT: Duration = Duration::from_secs(120);
/// Samples per second read for the waveform; plenty for a timeline.
const SAMPLE_RATE: usize = 2000;
/// Waveform columns per second of sound, before long files are thinned.
const PEAKS_PER_SECOND: usize = 50;
const MAX_PEAKS: usize = 12_000;
const MAX_STILLS: usize = 12;
const STILL_HEIGHT: u32 = 72;
const FFMPEG_HINT: &str = "Install FFmpeg with `brew install ffmpeg`.";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Peaks {
    /// Seconds of sound each value covers.
    pub step: f64,
    /// Loudness of each step, 0 to 255.
    pub values: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Still {
    /// Seconds into the file.
    pub time: f64,
    /// Full path of the image.
    pub path: String,
}

pub fn root(data_dir: &Path) -> PathBuf {
    data_dir.join("media-cache")
}

/// A name for this version of the file: its path, size and modification time.
fn key(file: &Path) -> AppResult<String> {
    let meta = std::fs::metadata(file).map_err(|_| AppError::NotFound("That media file is not in the project any more.".into()))?;
    let modified = meta.modified().ok().and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis());
    let identity = format!("{}|{}|{modified}", file.display(), meta.len());
    Ok(format!("{:016x}", ids::fnv1a(identity.as_bytes())))
}

pub async fn peaks(data_dir: &Path, project: &Path, src: &str) -> AppResult<Peaks> {
    let file = media::resolve(project, src)?;
    let cache = root(data_dir).join("peaks").join(format!("{}.json", key(&file)?));
    if let Ok(text) = std::fs::read_to_string(&cache) {
        if let Ok(found) = serde_json::from_str::<Peaks>(&text) {
            return Ok(found);
        }
    }
    let ffmpeg = shell_env::require_program("ffmpeg", FFMPEG_HINT)?;
    let mut command = shell_env::command(&ffmpeg);
    command
        .args(["-v", "error", "-i"])
        .arg(&file)
        .args(["-vn", "-ac", "1", "-ar", &SAMPLE_RATE.to_string(), "-f", "s16le", "-"]);
    let output = shell_env::run(command, LIMIT, "ffmpeg").await?;
    if !output.status.success() {
        return Err(AppError::ToolFailed(format!("The sound could not be read.\n{}", shell_env::failure_text(&output))));
    }
    let made = bucket(&output.stdout);
    store::write_atomic(&cache, &made)?;
    Ok(made)
}

/// Turns 16-bit mono samples into one loudness value per step.
fn bucket(pcm: &[u8]) -> Peaks {
    let samples: Vec<i16> = pcm.chunks_exact(2).map(|pair| i16::from_le_bytes([pair[0], pair[1]])).collect();
    let mut per = SAMPLE_RATE / PEAKS_PER_SECOND;
    while samples.len() / per.max(1) > MAX_PEAKS {
        per *= 2;
    }
    let values = samples
        .chunks(per.max(1))
        .map(|chunk| {
            let loudest = chunk.iter().map(|sample| sample.unsigned_abs()).max().unwrap_or(0);
            // A square root lifts quiet speech so it is still visible.
            ((f64::from(loudest) / 32768.0).sqrt() * 255.0).round().min(255.0) as u8
        })
        .collect();
    Peaks { step: per as f64 / SAMPLE_RATE as f64, values }
}

pub async fn stills(data_dir: &Path, project: &Path, src: &str) -> AppResult<Vec<Still>> {
    let file = media::resolve(project, src)?;
    let dir = root(data_dir).join("stills").join(key(&file)?);
    let index = dir.join("index.json");
    if let Ok(text) = std::fs::read_to_string(&index) {
        if let Ok(found) = serde_json::from_str::<Vec<Still>>(&text) {
            if found.iter().all(|still| Path::new(&still.path).is_file()) {
                return Ok(found);
            }
        }
    }
    let duration = probe_seconds(&file).await?;
    let count = ((duration / 2.0).ceil() as usize).clamp(1, MAX_STILLS);
    std::fs::create_dir_all(&dir)?;
    let ffmpeg = shell_env::require_program("ffmpeg", FFMPEG_HINT)?;
    let mut found = Vec::new();
    for index in 0..count {
        let time = duration * (index as f64 + 0.5) / count as f64;
        let path = dir.join(format!("{index:02}.jpg"));
        let mut command = shell_env::command(&ffmpeg);
        command
            .args(["-y", "-v", "error", "-ss", &format!("{time:.3}"), "-i"])
            .arg(&file)
            .args(["-frames:v", "1", "-vf", &format!("scale=-2:{STILL_HEIGHT}"), "-q:v", "5"])
            .arg(&path);
        let output = shell_env::run(command, LIMIT, "ffmpeg").await?;
        if output.status.success() && path.is_file() {
            found.push(Still { time, path: path.to_string_lossy().into_owned() });
        }
    }
    if found.is_empty() {
        return Err(AppError::ToolFailed("No stills could be taken from that video.".into()));
    }
    store::write_atomic(&index, &found)?;
    Ok(found)
}

async fn probe_seconds(file: &Path) -> AppResult<f64> {
    let ffprobe = shell_env::require_program("ffprobe", FFMPEG_HINT)?;
    let mut command = shell_env::command(&ffprobe);
    command.args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1"]).arg(file);
    let output = shell_env::run(command, Duration::from_secs(30), "ffprobe").await?;
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|seconds| *seconds > 0.0)
        .ok_or_else(|| AppError::ToolFailed("The video's length could not be read.".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaks_follow_the_loudest_sample_of_each_step() {
        let per = SAMPLE_RATE / PEAKS_PER_SECOND;
        let mut samples = vec![0i16; per * 3];
        samples[per + 5] = i16::MAX;
        samples[2 * per] = -8192;
        let pcm: Vec<u8> = samples.iter().flat_map(|sample| sample.to_le_bytes()).collect();
        let made = bucket(&pcm);
        assert_eq!(made.values.len(), 3);
        assert_eq!(made.values[0], 0);
        assert_eq!(made.values[1], 255);
        assert_eq!(made.values[2], 128, "a quarter of full scale shows as half height");
        assert!((made.step - 0.02).abs() < 1e-9);
    }

    #[test]
    fn long_sound_is_thinned_to_the_cap() {
        let pcm = vec![0u8; SAMPLE_RATE * 2 * 600];
        let made = bucket(&pcm);
        assert!(made.values.len() <= MAX_PEAKS);
        assert!((made.step * made.values.len() as f64 - 600.0).abs() < 1.0);
    }
}
