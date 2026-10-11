//! Music beds: original instrumental sound files synthesised on this Mac.
//!
//! The synthesis is a Python script carried inside the app
//! (`resources/music_bed.py`). It runs in the app's own Python environment,
//! the one the voiceover install creates, which already has numpy.

use std::path::{Path, PathBuf};

use tokio::process::Command;

use crate::error::{AppError, AppResult};
use crate::{media, shell_env, voice};

const SCRIPT: &str = include_str!("../resources/music_bed.py");

/// The limits the script enforces, repeated here so a request is refused
/// before it is queued. Keep in step with `music_bed.py`.
const MIN_SECONDS: f64 = 3.0;
const MAX_SECONDS: f64 = 600.0;
const MIN_BEFORE_IMPACT: f64 = 3.0;
const MIN_AFTER_IMPACT: f64 = 2.0;

/// The styles, with the name each is shown by.
const STYLES: [(&str, &str); 4] =
    [("calm", "Calm"), ("steady", "Steady"), ("bright", "Bright"), ("cinematic", "Cinematic build")];

/// What a request for a bed comes to: where it will be written, the
/// arguments for the script, and a label for the jobs list.
#[derive(Debug)]
pub struct MusicPlan {
    pub output: PathBuf,
    pub args: Vec<String>,
    pub label: String,
}

pub fn plan(style: &str, seconds: f64, impact_at: Option<f64>, project: &Path) -> AppResult<MusicPlan> {
    let name = STYLES
        .iter()
        .find(|(id, _)| *id == style)
        .map(|(_, name)| *name)
        .ok_or_else(|| AppError::InvalidInput("That is not a music style the app has.".into()))?;
    if !seconds.is_finite() || !(MIN_SECONDS..=MAX_SECONDS).contains(&seconds) {
        return Err(AppError::InvalidInput(format!(
            "A music bed can be {MIN_SECONDS:.0} to {MAX_SECONDS:.0} seconds long."
        )));
    }
    let length = trimmed(seconds);
    let folder = media::assets_dir(project).join("music");
    let output = media::free_name(&folder, Path::new(&format!("{style}-{length}s.wav")));
    let mut args = vec![style.to_owned(), length.clone(), output.to_string_lossy().into_owned()];

    if style == "cinematic" {
        let impact = impact_at.filter(|at| at.is_finite()).ok_or_else(|| {
            AppError::InvalidInput("Say at which second the impact should land.".into())
        })?;
        if impact < MIN_BEFORE_IMPACT || impact > seconds - MIN_AFTER_IMPACT {
            return Err(AppError::InvalidInput(format!(
                "The impact must be at least {MIN_BEFORE_IMPACT:.0} seconds in and {MIN_AFTER_IMPACT:.0} seconds before the end."
            )));
        }
        args.push(trimmed(impact));
    }
    Ok(MusicPlan { output, args, label: format!("Music: {name}, {length} s") })
}

/// A number of seconds without needless zeros: 24, 12.5.
fn trimmed(seconds: f64) -> String {
    let text = format!("{seconds:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// The script, written afresh from the copy inside the app.
fn script(data_dir: &Path) -> AppResult<PathBuf> {
    let dir = data_dir.join("tools");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("music_bed.py");
    std::fs::write(&path, SCRIPT)?;
    Ok(path)
}

/// The process that synthesises a bed, ready to spawn.
pub fn command(data_dir: &Path, args: &[String], cwd: &Path) -> AppResult<Command> {
    let python = voice::python(data_dir).ok_or_else(|| {
        AppError::ToolMissing("Music needs a one-time install. Open Check Setup and choose Install Voiceover and Music.".into())
    })?;
    let mut command = shell_env::command(&python);
    // -I keeps the user's own Python settings and the project folder out of it.
    command.arg("-I").arg(script(data_dir)?).args(args).current_dir(cwd);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bed_is_named_for_its_style_and_length_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let first = plan("steady", 24.0, None, dir.path()).unwrap();
        assert_eq!(first.output, dir.path().join("assets/music/steady-24s.wav"));
        assert_eq!(first.args[..2], ["steady", "24"]);
        assert_eq!(first.args.len(), 3);
        assert_eq!(first.label, "Music: Steady, 24 s");

        std::fs::create_dir_all(first.output.parent().unwrap()).unwrap();
        std::fs::write(&first.output, b"x").unwrap();
        let second = plan("steady", 24.0, Some(9.0), dir.path()).unwrap();
        assert_eq!(second.output, dir.path().join("assets/music/steady-24s-2.wav"));
        assert_eq!(second.args.len(), 3, "an impact is only for the cinematic style");
    }

    #[test]
    fn the_cinematic_style_needs_an_impact_with_room_around_it() {
        let dir = tempfile::tempdir().unwrap();
        let built = plan("cinematic", 12.5, Some(7.0), dir.path()).unwrap();
        assert_eq!(built.args[..2], ["cinematic", "12.5"]);
        assert_eq!(built.args[3], "7");
        assert_eq!(built.label, "Music: Cinematic build, 12.5 s");
        for impact in [None, Some(2.0), Some(11.0), Some(f64::NAN)] {
            assert_eq!(plan("cinematic", 12.5, impact, dir.path()).unwrap_err().code(), "invalid_input", "{impact:?}");
        }
    }

    #[test]
    fn unknown_styles_and_odd_lengths_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(plan("jazz", 10.0, None, dir.path()).unwrap_err().code(), "invalid_input");
        assert_eq!(plan("--help", 10.0, None, dir.path()).unwrap_err().code(), "invalid_input");
        for seconds in [0.0, 2.9, 601.0, f64::INFINITY, f64::NAN] {
            assert_eq!(plan("calm", seconds, None, dir.path()).unwrap_err().code(), "invalid_input", "{seconds}");
        }
    }

    #[test]
    fn without_the_python_environment_the_job_says_what_to_install() {
        let data = tempfile::tempdir().unwrap();
        let error = command(data.path(), &[], data.path()).unwrap_err();
        assert_eq!(error.code(), "tool_missing");
        assert!(error.to_string().contains("Install Voiceover and Music"));
    }

    #[test]
    fn the_script_is_written_from_the_apps_own_copy() {
        let data = tempfile::tempdir().unwrap();
        let path = script(data.path()).unwrap();
        std::fs::write(&path, "tampered").unwrap();
        let again = script(data.path()).unwrap();
        assert_eq!(std::fs::read_to_string(again).unwrap(), SCRIPT);
        assert!(SCRIPT.contains("def cinematic(") && SCRIPT.contains("\"steady\""));
    }
}
