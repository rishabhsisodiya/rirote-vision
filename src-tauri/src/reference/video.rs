//! Looking at the reference video itself: its size and length, where the cuts
//! are, and one still from the middle of each shot.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use super::model::{Shot, ShotList};
use super::run::{self, Cancel};
use crate::error::{AppError, AppResult};
use crate::shell_env;

const PROBE_LIMIT: Duration = Duration::from_secs(60);
const SCAN_LIMIT: Duration = Duration::from_secs(15 * 60);
const STILL_LIMIT: Duration = Duration::from_secs(60);
/// How different two frames must be to count as a cut (ffmpeg's scene score, 0–1).
pub const CUT_THRESHOLD: f64 = 0.3;
/// Cuts closer than this are flashes inside one shot, not separate shots.
const MIN_SHOT: f64 = 0.25;
/// Analysis looks at no more stills than this; long references are sampled.
pub const MAX_STILLS: usize = 60;

const FFMPEG_HINT: &str = "Install FFmpeg with `brew install ffmpeg`.";

#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    pub duration: f64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
}

pub async fn probe(video: &Path, cancel: &Cancel) -> AppResult<Probe> {
    let ffprobe = shell_env::require_program("ffprobe", FFMPEG_HINT)?;
    let mut command = shell_env::command(&ffprobe);
    command.args(["-v", "error", "-print_format", "json", "-show_streams", "-show_format"]).arg(video);
    let output = run::run(command, PROBE_LIMIT, "ffprobe", cancel).await?;
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| AppError::ToolFailed(format!("The video could not be read.\n{}", shell_env::failure_text(&output))))?;
    parse_probe(&value).ok_or_else(|| AppError::InvalidInput("That file has no picture to study.".into()))
}

fn parse_probe(value: &Value) -> Option<Probe> {
    let streams = value.get("streams")?.as_array()?;
    let video = streams.iter().find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"))?;
    let number = |v: Option<&Value>| v.and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_f64()));
    let duration = number(value.get("format").and_then(|format| format.get("duration")))
        .or_else(|| number(video.get("duration")))
        .unwrap_or(0.0);
    let fps = video
        .get("avg_frame_rate")
        .and_then(Value::as_str)
        .and_then(|rate| {
            let (top, bottom) = rate.split_once('/')?;
            let (top, bottom): (f64, f64) = (top.parse().ok()?, bottom.parse().ok()?);
            (bottom > 0.0).then(|| top / bottom)
        })
        .unwrap_or(0.0);
    Some(Probe {
        duration,
        fps: (fps * 1000.0).round() / 1000.0,
        width: video.get("width").and_then(Value::as_u64).unwrap_or(0) as u32,
        height: video.get("height").and_then(Value::as_u64).unwrap_or(0) as u32,
    })
}

/// Finds the cuts with ffmpeg's scene score and turns them into shots.
pub async fn shots(video: &Path, probe: &Probe, cancel: &Cancel) -> AppResult<ShotList> {
    let ffmpeg = shell_env::require_program("ffmpeg", FFMPEG_HINT)?;
    let mut command = shell_env::command(&ffmpeg);
    command
        .args(["-hide_banner", "-nostats", "-i"])
        .arg(video)
        .args(["-an", "-vf", &format!("select='gt(scene,{CUT_THRESHOLD})',showinfo"), "-f", "null", "-"]);
    let output = run::run(command, SCAN_LIMIT, "ffmpeg", cancel).await?;
    if !output.status.success() {
        return Err(AppError::ToolFailed(format!("The cuts could not be found.\n{}", shell_env::failure_text(&output))));
    }
    let cuts = cut_times(&String::from_utf8_lossy(&output.stderr));
    Ok(ShotList {
        video_duration: round(probe.duration),
        fps: probe.fps,
        threshold: CUT_THRESHOLD,
        shots: shots_from_cuts(&cuts, probe.duration),
    })
}

fn cut_times(log: &str) -> Vec<f64> {
    log.split("pts_time:")
        .skip(1)
        .filter_map(|rest| rest.split_whitespace().next()?.parse().ok())
        .collect()
}

fn shots_from_cuts(cuts: &[f64], duration: f64) -> Vec<Shot> {
    let mut edges = vec![0.0];
    for &cut in cuts {
        let last = *edges.last().unwrap_or(&0.0);
        if cut - last >= MIN_SHOT && duration - cut >= MIN_SHOT {
            edges.push(cut);
        }
    }
    edges.push(duration.max(0.0));
    edges
        .windows(2)
        .enumerate()
        .map(|(index, pair)| Shot {
            index,
            start: round(pair[0]),
            end: round(pair[1]),
            duration: round(pair[1] - pair[0]),
            keyframe: None,
        })
        .collect()
}

fn round(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// Which shots get a still: all of them, or an even spread of `MAX_STILLS`.
pub fn sampled(count: usize) -> Vec<usize> {
    if count <= MAX_STILLS {
        return (0..count).collect();
    }
    (0..MAX_STILLS).map(|i| i * (count - 1) / (MAX_STILLS - 1)).collect()
}

/// One still from the middle of each sampled shot, into `keyframes/`.
pub async fn keyframes(video: &Path, dir: &Path, list: &mut ShotList, cancel: &Cancel) -> AppResult<()> {
    let ffmpeg = shell_env::require_program("ffmpeg", FFMPEG_HINT)?;
    std::fs::create_dir_all(dir.join("keyframes"))?;
    for index in sampled(list.shots.len()) {
        let shot = &mut list.shots[index];
        let relative = format!("keyframes/shot_{:03}.jpg", shot.index);
        let middle = shot.start + shot.duration / 2.0;
        let mut command = shell_env::command(&ffmpeg);
        command
            .args(["-y", "-v", "error", "-ss", &format!("{middle:.3}"), "-i"])
            .arg(video)
            .args(["-frames:v", "1", "-vf", "scale='min(720,iw)':-2", "-q:v", "3"])
            .arg(dir.join(&relative));
        let output = run::run(command, STILL_LIMIT, "ffmpeg", cancel).await?;
        if !output.status.success() || !dir.join(&relative).is_file() {
            return Err(AppError::ToolFailed(format!(
                "A still from {middle:.1} s could not be taken.\n{}",
                shell_env::failure_text(&output)
            )));
        }
        shot.keyframe = Some(relative);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_size_length_and_rate() {
        let value = serde_json::json!({
            "streams": [{"codec_type": "audio"}, {"codec_type": "video", "width": 1080, "height": 1920, "avg_frame_rate": "30000/1001"}],
            "format": {"duration": "31.50"}
        });
        assert_eq!(parse_probe(&value), Some(Probe { duration: 31.5, fps: 29.97, width: 1080, height: 1920 }));
        assert_eq!(parse_probe(&serde_json::json!({"streams": [{"codec_type": "audio"}]})), None);
    }

    #[test]
    fn cuts_become_shots_and_flashes_are_ignored() {
        let log = "[Parsed_showinfo_1] n:0 pts:50 pts_time:2 duration\n[x] n:1 pts_time:2.1 \n[x] pts_time:6.75 dur\n[x] pts_time:9.9";
        assert_eq!(cut_times(log), vec![2.0, 2.1, 6.75, 9.9]);
        let shots = shots_from_cuts(&cut_times(log), 10.0);
        let spans: Vec<(f64, f64)> = shots.iter().map(|shot| (shot.start, shot.end)).collect();
        assert_eq!(spans, vec![(0.0, 2.0), (2.0, 6.75), (6.75, 10.0)]);
        assert_eq!(shots_from_cuts(&[], 4.0).len(), 1, "no cuts is one shot");
    }

    #[test]
    fn long_references_are_sampled_evenly_with_both_ends() {
        assert_eq!(sampled(3), vec![0, 1, 2]);
        let picks = sampled(200);
        assert_eq!(picks.len(), MAX_STILLS);
        assert_eq!((picks[0], *picks.last().unwrap()), (0, 199));
    }
}
