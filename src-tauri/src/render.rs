//! Export: turning the composition into a video file.
//!
//! This module decides what an export is (its options, its output file, the
//! engine command), checks a composition before it is exported, and looks at
//! the file afterwards. Running the export is the jobs queue's work.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::{hyperframes, media, shell_env};

const CHECK_TIMEOUT: Duration = Duration::from_secs(300);
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_NAME: usize = 80;
/// Problems listed before an export; the rest are counted.
const MAX_FINDINGS: usize = 12;
/// How far the file's length may be from the composition's before it is remarked on.
const LENGTH_TOLERANCE: f64 = 0.25;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Mp4,
    Mov,
    Webm,
    Gif,
}

impl Format {
    fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mov => "mov",
            Self::Webm => "webm",
            Self::Gif => "gif",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    Draft,
    Standard,
    Delivery,
}

impl Quality {
    /// The engine's name for the preset.
    fn flag(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Standard => "looks",
            Self::Delivery => "delivery",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    pub format: Format,
    pub quality: Quality,
    /// Frames per second; the composition's own rate when absent.
    pub fps: Option<u32>,
    /// A larger size of the same shape, such as `portrait-4k`.
    pub resolution: Option<String>,
    /// The file's name, without its extension.
    pub name: String,
    /// Where to save; the project's `renders` folder when absent.
    pub folder: Option<String>,
}

const FRAME_RATES: [u32; 5] = [24, 25, 30, 50, 60];
const RESOLUTIONS: [&str; 3] = ["landscape-4k", "portrait-4k", "square-4k"];

/// The file an export will write, and the engine command that writes it.
pub fn plan(options: &ExportOptions, project: &Path) -> AppResult<(PathBuf, Vec<String>)> {
    let name = file_name(&options.name)?;
    let folder = match &options.folder {
        None => project.join("renders"),
        Some(folder) => {
            let folder = PathBuf::from(folder);
            if !folder.is_absolute() || !folder.is_dir() {
                return Err(AppError::NotFound(format!("The folder {} does not exist.", folder.display())));
            }
            folder
        }
    };
    if let Some(fps) = options.fps {
        if !FRAME_RATES.contains(&fps) {
            return Err(AppError::InvalidInput("That frame rate is not offered.".into()));
        }
    }
    if let Some(resolution) = &options.resolution {
        if !RESOLUTIONS.contains(&resolution.as_str()) {
            return Err(AppError::InvalidInput("That size is not offered.".into()));
        }
    }

    // Never over a file that is already there: `film.mp4`, then `film-2.mp4`.
    let extension = options.format.extension();
    let output = media::free_name(&folder, Path::new(&format!("{name}.{extension}")));
    let mut args: Vec<String> = ["render", "-o"].map(String::from).to_vec();
    args.push(output.to_string_lossy().into_owned());
    args.extend(["-q", options.quality.flag(), "--format", extension].map(String::from));
    if let Some(fps) = options.fps {
        args.extend(["--fps".to_owned(), fps.to_string()]);
    }
    if let Some(resolution) = &options.resolution {
        args.extend(["--resolution".to_owned(), resolution.clone()]);
    }
    Ok((output, args))
}

/// A name that is one file name and nothing else.
fn file_name(name: &str) -> AppResult<String> {
    let name = name.trim().trim_end_matches('.');
    let plain = name.chars().all(|ch| ch.is_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.' | '(' | ')'));
    if name.is_empty() || name.starts_with('.') || name.starts_with('-') || !plain {
        return Err(AppError::InvalidInput(
            "Name the file with letters, numbers, spaces, hyphens or underscores.".into(),
        ));
    }
    if name.chars().count() > MAX_NAME {
        return Err(AppError::InvalidInput(format!("A file name can be at most {MAX_NAME} characters.")));
    }
    Ok(name.to_owned())
}

/// What the engine says while rendering, in the editor's words.
pub fn stage(code: &str, done: Option<u64>, total: Option<u64>) -> String {
    let counted = |what: &str| match (done, total) {
        (Some(done), Some(total)) if total > 0 => format!("{what} {done} of {total}"),
        _ => what.to_owned(),
    };
    match code {
        "compile" => "Preparing the composition".into(),
        "extract_video" => "Reading video".into(),
        "process_audio" => "Processing sound".into(),
        "start_browsers" => "Starting capture".into(),
        "capture" => counted("Capturing frame"),
        "encode" => counted("Encoding frame"),
        "assemble" => "Assembling the file".into(),
        _ => "Rendering".into(),
    }
}

/// A problem the engine found in the composition.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Finding {
    /// lint, runtime, layout, motion or contrast.
    pub area: String,
    pub message: String,
    pub hint: Option<String>,
    /// The moment in the film it was seen, in seconds.
    pub at: Option<f64>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct CheckSummary {
    /// Nothing stands in the way of an export.
    pub ok: bool,
    pub errors: Vec<Finding>,
    /// Errors beyond the ones listed.
    pub more_errors: usize,
    pub warnings: usize,
}

/// Runs the engine's checks on the composition.
pub async fn check(data_dir: &Path, project: &Path) -> AppResult<CheckSummary> {
    let report: Value =
        hyperframes::run_json(data_dir, &["check", "--json"], project, CHECK_TIMEOUT).await?;
    Ok(summarise(&report))
}

fn summarise(report: &Value) -> CheckSummary {
    let mut errors = Vec::new();
    let mut warnings = 0;
    let mut total_errors = 0;
    for area in ["lint", "runtime", "layout", "motion", "contrast"] {
        let section = &report[area];
        warnings += section["warningCount"].as_u64().unwrap_or(0) as usize;
        total_errors += section["errorCount"].as_u64().unwrap_or(0) as usize;
        for finding in section["findings"].as_array().into_iter().flatten() {
            if finding["severity"] == "error" && errors.len() < MAX_FINDINGS {
                errors.push(Finding {
                    area: area.to_owned(),
                    message: shell_env::first_line(finding["message"].as_str().unwrap_or("A problem was found.")),
                    hint: finding["fixHint"].as_str().map(shell_env::first_line).filter(|hint| !hint.is_empty()),
                    at: finding["time"].as_f64(),
                });
            }
        }
    }
    let total_errors = total_errors.max(errors.len());
    CheckSummary {
        ok: total_errors == 0 && report["ok"].as_bool().unwrap_or(true),
        more_errors: total_errors - errors.len(),
        errors,
        warnings,
    }
}

/// Looks at an exported file. Returns a line describing it, or why it is
/// not a usable video.
pub async fn inspect(output: &Path, expected_length: Option<f64>) -> AppResult<String> {
    let bytes = std::fs::metadata(output).map(|meta| meta.len()).unwrap_or(0);
    if bytes == 0 {
        return Err(AppError::ToolFailed("The exported file is empty.".into()));
    }
    let Some(ffprobe) = shell_env::find_program("ffprobe") else {
        return Ok(size_text(bytes));
    };
    let mut command = shell_env::command(&ffprobe);
    command
        .args(["-v", "error", "-select_streams", "v:0"])
        .args(["-show_entries", "stream=width,height:format=duration", "-of", "json"])
        .arg(output);
    let probed = shell_env::run(command, PROBE_TIMEOUT, "ffprobe").await?;
    let facts: Value = serde_json::from_slice(&probed.stdout).unwrap_or(Value::Null);
    describe(&facts, bytes, expected_length)
}

fn describe(facts: &Value, bytes: u64, expected_length: Option<f64>) -> AppResult<String> {
    let stream = &facts["streams"][0];
    let (Some(width), Some(height)) = (stream["width"].as_u64(), stream["height"].as_u64()) else {
        return Err(AppError::ToolFailed("The exported file has no picture in it.".into()));
    };
    let length = facts["format"]["duration"].as_str().and_then(|text| text.parse::<f64>().ok());
    let mut parts = vec![format!("{width} × {height}")];
    if let Some(length) = length {
        parts.push(format!("{length:.1} s"));
    }
    parts.push(size_text(bytes));
    let mut line = parts.join(" · ");
    if let (Some(length), Some(expected)) = (length, expected_length) {
        if (length - expected).abs() > LENGTH_TOLERANCE {
            line.push_str(&format!(". The composition is {expected:.1} s long, so check the end of the file"));
        }
    }
    Ok(line)
}

fn size_text(bytes: u64) -> String {
    match bytes {
        0..=1023 => format!("{bytes} B"),
        1024..=1_048_575 => format!("{:.0} KB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn options(name: &str) -> ExportOptions {
        ExportOptions {
            format: Format::Mp4,
            quality: Quality::Standard,
            fps: None,
            resolution: None,
            name: name.into(),
            folder: None,
        }
    }

    #[test]
    fn an_export_goes_to_the_renders_folder_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let (output, args) = plan(&options("Launch film"), dir.path()).unwrap();
        assert_eq!(output, dir.path().join("renders/Launch film.mp4"));
        assert_eq!(args[0], "render");
        assert_eq!(&args[3..], ["-q", "looks", "--format", "mp4"]);

        std::fs::create_dir_all(dir.path().join("renders")).unwrap();
        std::fs::write(&output, b"x").unwrap();
        let (second, _) = plan(&options("Launch film"), dir.path()).unwrap();
        assert_eq!(second, dir.path().join("renders/Launch film-2.mp4"));
    }

    #[test]
    fn options_become_engine_flags() {
        let dir = tempfile::tempdir().unwrap();
        let chosen = ExportOptions {
            format: Format::Mov,
            quality: Quality::Delivery,
            fps: Some(60),
            resolution: Some("portrait-4k".into()),
            name: "reel_v2".into(),
            folder: Some(dir.path().to_string_lossy().into_owned()),
        };
        let (output, args) = plan(&chosen, Path::new("/project")).unwrap();
        assert_eq!(output, dir.path().join("reel_v2.mov"));
        assert_eq!(&args[3..], ["-q", "delivery", "--format", "mov", "--fps", "60", "--resolution", "portrait-4k"]);
    }

    #[test]
    fn odd_names_rates_sizes_and_folders_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["", "  ", "../escape", "a/b", ".hidden", "-o", "x\ny", &"n".repeat(81)] {
            assert_eq!(plan(&options(name), dir.path()).unwrap_err().code(), "invalid_input", "{name:?}");
        }
        let mut odd = options("film");
        odd.fps = Some(23);
        assert_eq!(plan(&odd, dir.path()).unwrap_err().code(), "invalid_input");
        let mut odd = options("film");
        odd.resolution = Some("--docker".into());
        assert_eq!(plan(&odd, dir.path()).unwrap_err().code(), "invalid_input");
        let mut odd = options("film");
        odd.folder = Some(dir.path().join("gone").to_string_lossy().into_owned());
        assert_eq!(plan(&odd, dir.path()).unwrap_err().code(), "not_found");
    }

    #[test]
    fn render_stages_are_put_in_words() {
        assert_eq!(stage("capture", Some(120), Some(300)), "Capturing frame 120 of 300");
        assert_eq!(stage("assemble", Some(0), Some(10)), "Assembling the file");
        assert_eq!(stage("compile", None, None), "Preparing the composition");
        assert_eq!(stage("something_new", None, None), "Rendering");
    }

    #[test]
    fn a_clean_check_is_ok_and_counts_warnings() {
        let report = json!({ "ok": true,
            "lint": { "errorCount": 0, "warningCount": 25, "findings": [{ "severity": "warning", "message": "w" }] },
            "contrast": { "errorCount": 0, "warningCount": 2, "findings": [] } });
        let summary = summarise(&report);
        assert!(summary.ok);
        assert_eq!((summary.errors.len(), summary.more_errors, summary.warnings), (0, 0, 27));
    }

    #[test]
    fn errors_are_listed_with_where_and_how_to_fix() {
        let finding = |n: usize| json!({ "severity": "error", "message": format!("Clip {n} overflows the frame\nmore"), "fixHint": "Shrink it", "time": 2.5 });
        let report = json!({ "ok": false,
            "layout": { "errorCount": 15, "warningCount": 0, "findings": (0..15).map(finding).collect::<Vec<_>>() },
            "runtime": { "errorCount": 1, "warningCount": 0, "findings": [{ "severity": "error", "message": "Missing asset" }] } });
        let summary = summarise(&report);
        assert!(!summary.ok);
        assert_eq!(summary.errors.len(), MAX_FINDINGS);
        assert_eq!(summary.more_errors, 16 - MAX_FINDINGS);
        assert_eq!(summary.errors[0].area, "runtime");
        assert_eq!(summary.errors[0].hint, None);
        assert_eq!(summary.errors[1], Finding { area: "layout".into(), message: "Clip 0 overflows the frame".into(), hint: Some("Shrink it".into()), at: Some(2.5) });
    }

    #[test]
    fn an_exported_file_is_described_or_found_wanting() {
        let facts = json!({ "streams": [{ "width": 1080, "height": 1920 }], "format": { "duration": "24.452000" } });
        assert_eq!(describe(&facts, 3_500_000, Some(24.452)).unwrap(), "1080 × 1920 · 24.5 s · 3.3 MB");
        let short = describe(&facts, 3_500_000, Some(30.0)).unwrap();
        assert!(short.contains("The composition is 30.0 s long"), "{short}");
        assert_eq!(describe(&json!({ "streams": [] }), 10, None).unwrap_err().code(), "tool_failed");
        assert_eq!(size_text(900), "900 B");
        assert_eq!(size_text(32_858), "32 KB");
    }
}
