//! The timeline: a project's clips read as data, and the edits made to them.
//!
//! Reading and editing both go through the engine's `timeline` commands, so
//! the app never rewrites composition files itself. Every edit the engine
//! makes returns a receipt that can be undone; the app keeps those receipts
//! as its undo and redo history.
//!
//! A receipt is undone from a backup the engine made, and the engine keeps
//! only its ten newest backups of a file. So that undo reaches further back
//! than that, the app keeps its own copy of each backup and puts it back
//! where the engine looks when the engine's copy has gone.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::error::{AppError, AppResult};
use crate::{hyperframes, shell_env};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
/// The shortest clip an edit may leave behind, in seconds.
const MIN_CLIP: f64 = 0.05;
const MAX_HISTORY: usize = 100;
const STALE_HISTORY: &str =
    "The project changed outside the timeline, so earlier edits can no longer be undone.";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    /// How the engine addresses the clip, for example `#intro`.
    #[serde(rename = "ref")]
    pub reference: String,
    pub label: Option<String>,
    /// video, graphics, captions or audio.
    pub kind: String,
    pub track: i64,
    /// Seconds on the main timeline.
    pub start: f64,
    pub end: f64,
    pub src: Option<String>,
    /// How loud it plays, 1 being as recorded. None for clips without sound.
    pub volume: Option<f64>,
    /// Plays a media file, so its start cannot simply be re-timed.
    pub media: bool,
    /// Clips inside a sub-composition are shown but not edited here.
    pub editable: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Timeline {
    pub duration: f64,
    pub clips: Vec<Clip>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineState {
    pub timeline: Timeline,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum EditOp {
    /// Slide the clip so it begins at `start`.
    Move { clip: String, start: f64 },
    /// Drag the clip's left edge to `start`; its end stays where it is.
    TrimStart { clip: String, start: f64 },
    /// Drag the clip's right edge to `end`.
    TrimEnd { clip: String, end: f64 },
    /// Cut the clip in two at `time`.
    Split { clip: String, time: f64 },
    Delete { clip: String },
    Duplicate { clip: String },
    /// Put the clip on `track`, beginning at `start`.
    Place { clip: String, start: f64, track: i64 },
    /// Set how loud the clip plays; 1 is as recorded.
    SetVolume { clip: String, volume: f64 },
}

impl EditOp {
    fn clip(&self) -> &str {
        match self {
            Self::Move { clip, .. }
            | Self::TrimStart { clip, .. }
            | Self::TrimEnd { clip, .. }
            | Self::Split { clip, .. }
            | Self::Delete { clip }
            | Self::Duplicate { clip }
            | Self::Place { clip, .. }
            | Self::SetVolume { clip, .. } => clip,
        }
    }
}

/// One undoable step: the receipts of the engine edits it was made of, in
/// the order they were made.
type Entry = Vec<Value>;

#[derive(Default)]
struct History {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
}

/// The app's own copies of the engine's backups, for one session. They are
/// small (one composition file each) and are cleared when the app starts.
struct BackupStore {
    dir: PathBuf,
}

impl BackupStore {
    fn copy_of(&self, project: &Path, receipt: &Value) -> Option<PathBuf> {
        let name = Path::new(receipt["backupPath"].as_str()?).file_name()?;
        Some(self.dir.join(crate::ids::project_key(project)).join(name))
    }

    /// Copies the backup a receipt points at, while the engine still has it.
    fn keep(&self, project: &Path, receipt: &Value) {
        let (Some(backup), Some(copy)) = (in_project(project, receipt), self.copy_of(project, receipt)) else {
            return;
        };
        if let Some(folder) = copy.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        let _ = std::fs::copy(backup, copy);
    }

    /// Makes sure the engine will find the backup a receipt points at.
    fn restore(&self, project: &Path, receipt: &Value) -> bool {
        let Some(backup) = in_project(project, receipt) else { return false };
        if backup.is_file() {
            return true;
        }
        let Some(copy) = self.copy_of(project, receipt).filter(|copy| copy.is_file()) else {
            return false;
        };
        if let Some(folder) = backup.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        std::fs::copy(copy, backup).is_ok()
    }

    fn has(&self, project: &Path, receipt: &Value) -> bool {
        in_project(project, receipt).is_some_and(|backup| backup.is_file())
            || self.copy_of(project, receipt).is_some_and(|copy| copy.is_file())
    }

    fn forget(&self, project: &Path) {
        let _ = std::fs::remove_dir_all(self.dir.join(crate::ids::project_key(project)));
    }
}

/// Where the engine keeps the backup a receipt names. A receipt naming a
/// path outside the project's backup folder is not trusted.
fn in_project(project: &Path, receipt: &Value) -> Option<PathBuf> {
    let relative = Path::new(receipt["backupPath"].as_str()?);
    let inside = relative.starts_with(".hyperframes/backup")
        && relative.components().all(|part| matches!(part, std::path::Component::Normal(_)));
    inside.then(|| project.join(relative))
}

pub struct TimelineService {
    /// Undo and redo per project folder. Also serialises edits: one at a time.
    histories: Mutex<HashMap<String, History>>,
    backups: BackupStore,
}

impl TimelineService {
    pub fn new(data_dir: &Path) -> Self {
        // History lives for one session, and so do the copies behind it.
        let dir = data_dir.join("undo");
        let _ = std::fs::remove_dir_all(&dir);
        Self { histories: Mutex::new(HashMap::new()), backups: BackupStore { dir } }
    }

    pub async fn read(&self, data_dir: &Path, project: &Path) -> AppResult<TimelineState> {
        let mut histories = self.histories.lock().await;
        let timeline = read_timeline(data_dir, project).await?;
        Ok(self.current(timeline, &mut histories, project))
    }

    /// The timeline with what can truly still be undone and redone.
    fn current(
        &self,
        timeline: Timeline,
        histories: &mut HashMap<String, History>,
        project: &Path,
    ) -> TimelineState {
        if let Some(history) = histories.get_mut(&key(project)) {
            let reachable = |entry: &Entry| entry.iter().all(|receipt| self.backups.has(project, receipt));
            history.undo.retain(reachable);
            history.redo.retain(reachable);
        }
        state(timeline, histories.get(&key(project)))
    }

    pub async fn edit(&self, data_dir: &Path, project: &Path, op: EditOp) -> AppResult<TimelineState> {
        let mut histories = self.histories.lock().await;
        let before = read_timeline(data_dir, project).await?;
        let clip = before
            .clips
            .iter()
            .find(|clip| clip.reference == op.clip())
            .ok_or_else(|| AppError::NotFound("That clip is no longer on the timeline.".into()))?;
        let steps = plan(&op, clip, &before.clips)?;

        let mut receipts = Entry::new();
        for step in &steps {
            match run_edit(data_dir, project, step).await {
                Ok(receipt) => {
                    self.backups.keep(project, &receipt);
                    receipts.push(receipt);
                }
                Err(error) => {
                    // Leave the project as it was before a half-made edit.
                    let _ = self.revert(data_dir, project, &receipts).await;
                    return Err(error);
                }
            }
        }

        let history = histories.entry(key(project)).or_default();
        history.undo.push(receipts);
        history.redo.clear();
        if history.undo.len() > MAX_HISTORY {
            history.undo.remove(0);
        }
        let timeline = read_timeline(data_dir, project).await?;
        Ok(self.current(timeline, &mut histories, project))
    }

    pub async fn undo(&self, data_dir: &Path, project: &Path) -> AppResult<TimelineState> {
        self.step_history(data_dir, project, Direction::Undo).await
    }

    pub async fn redo(&self, data_dir: &Path, project: &Path) -> AppResult<TimelineState> {
        self.step_history(data_dir, project, Direction::Redo).await
    }

    /// Undoing an entry yields the receipts that redo it, and the reverse.
    async fn step_history(
        &self,
        data_dir: &Path,
        project: &Path,
        direction: Direction,
    ) -> AppResult<TimelineState> {
        let mut histories = self.histories.lock().await;
        let history = histories.entry(key(project)).or_default();
        let entry = match direction {
            Direction::Undo => history.undo.pop(),
            Direction::Redo => history.redo.pop(),
        };
        if let Some(entry) = entry {
            match self.revert(data_dir, project, &entry).await {
                Ok(inverse) => match direction {
                    Direction::Undo => history.redo.push(inverse),
                    Direction::Redo => history.undo.push(inverse),
                },
                Err(_) => {
                    // The receipts no longer match the files; none of them will.
                    histories.remove(&key(project));
                    self.backups.forget(project);
                    return Err(AppError::Refused(STALE_HISTORY.into()));
                }
            }
        }
        let timeline = read_timeline(data_dir, project).await?;
        Ok(self.current(timeline, &mut histories, project))
    }

    /// Undoes the edits behind `receipts`, newest first, and returns the
    /// receipts that would put them back, in the order these undos were made.
    async fn revert(&self, data_dir: &Path, project: &Path, receipts: &[Value]) -> AppResult<Entry> {
        let mut inverse = Entry::new();
        for receipt in receipts.iter().rev() {
            self.backups.restore(project, receipt);
            let text = receipt.to_string();
            let args = ["timeline", "undo", text.as_str(), "--json"];
            let output = hyperframes::run(data_dir, &args, project, COMMAND_TIMEOUT).await?;
            let answer = answer_of(&output)?;
            if answer.get("ok").and_then(Value::as_bool) != Some(true) {
                return Err(AppError::Refused(refusal(&answer)));
            }
            let back = answer
                .get("receipt")
                .and_then(Value::as_array)
                .and_then(|list| list.first())
                .cloned()
                .ok_or_else(|| AppError::ToolFailed("The undo came back without a receipt.".into()))?;
            self.backups.keep(project, &back);
            inverse.push(back);
        }
        Ok(inverse)
    }
}

enum Direction {
    Undo,
    Redo,
}

fn key(project: &Path) -> String {
    project.to_string_lossy().into_owned()
}

fn state(timeline: Timeline, history: Option<&History>) -> TimelineState {
    TimelineState {
        timeline,
        can_undo: history.is_some_and(|history| !history.undo.is_empty()),
        can_redo: history.is_some_and(|history| !history.redo.is_empty()),
    }
}

/// The composition's length in seconds, when it can be read.
pub async fn duration(data_dir: &Path, project: &Path) -> Option<f64> {
    read_timeline(data_dir, project).await.ok().map(|timeline| timeline.duration)
}

async fn read_timeline(data_dir: &Path, project: &Path) -> AppResult<Timeline> {
    let json: Value =
        hyperframes::run_json(data_dir, &["timeline", "--json"], project, COMMAND_TIMEOUT).await?;
    parse_timeline(&json).ok_or_else(|| {
        AppError::ToolFailed("The engine's timeline was not in the expected form.".into())
    })
}

fn parse_timeline(json: &Value) -> Option<Timeline> {
    let timeline = json.get("timeline")?;
    let duration = timeline.get("duration")?.as_f64()?;
    let mut clips: Vec<Clip> = timeline
        .get("tracks")?
        .as_array()?
        .iter()
        .filter_map(|track| track.get("rows")?.as_array())
        .flatten()
        .filter_map(parse_clip)
        .collect();
    clips.sort_by(|a, b| a.start.total_cmp(&b.start).then_with(|| a.id.cmp(&b.id)));
    Some(Timeline { duration, clips })
}

fn parse_clip(row: &Value) -> Option<Clip> {
    let text = |key: &str| row.get(key).and_then(Value::as_str).map(str::to_owned);
    let number = |key: &str| row.get(key).and_then(Value::as_f64);
    let tag = text("kind").unwrap_or_default();
    Some(Clip {
        id: text("id")?,
        reference: text("ref")?,
        label: text("label").filter(|label| !label.trim().is_empty()),
        kind: text("trackKind").unwrap_or_else(|| "graphics".into()),
        track: row.get("trackIndex").and_then(Value::as_i64).unwrap_or(0),
        start: number("absStart").or_else(|| number("start"))?,
        end: number("absEnd").or_else(|| number("end"))?,
        src: text("src"),
        volume: number("volume").or(matches!(tag.as_str(), "audio" | "video").then_some(1.0)),
        media: matches!(tag.as_str(), "audio" | "video"),
        editable: !row.get("nested").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// The loudest a clip may be set: twice as recorded.
const MAX_VOLUME: f64 = 2.0;
const MAX_TRACK: i64 = 99;

/// The engine commands that carry out `op` on `clip`, as argument lists.
/// `others` is every clip on the timeline, for the checks the engine leaves
/// to its caller.
fn plan(op: &EditOp, clip: &Clip, others: &[Clip]) -> AppResult<Vec<Vec<String>>> {
    if !clip.editable {
        return Err(AppError::Refused(
            "This clip belongs to a nested composition and cannot be edited from here yet.".into(),
        ));
    }
    let reference = safe_reference(&clip.reference)?;
    let command = |name: &str, rest: &[String]| {
        let mut args = vec!["timeline".to_owned(), name.to_owned(), reference.clone()];
        args.extend_from_slice(rest);
        args.push("--json".into());
        args
    };

    let steps = match *op {
        EditOp::Move { start, .. } => vec![command("move", &[seconds(start)?])],
        EditOp::TrimEnd { end, .. } => {
            let end = finite(end)?;
            if end - clip.start < MIN_CLIP {
                return Err(too_short());
            }
            vec![command("trim", &["--end".into(), seconds(end)?])]
        }
        EditOp::TrimStart { start, .. } => {
            let start = finite(start)?.max(0.0);
            if clip.end - start < MIN_CLIP {
                return Err(too_short());
            }
            if !clip.media {
                let length = seconds(clip.end - start)?;
                vec![command("trim", &["--start".into(), seconds(start)?, "--duration".into(), length])]
            } else if start > clip.start + MIN_CLIP {
                // Cutting keeps the sound or picture in step: the engine moves
                // the media's in-point for the part that remains.
                vec![command("split", &[seconds(start)?]), command("delete", &[])]
            } else if start < clip.start {
                return Err(AppError::Refused(
                    "The start of a media clip can only be trimmed inward for now.".into(),
                ));
            } else {
                return Err(too_short());
            }
        }
        EditOp::Split { time, .. } => {
            let time = finite(time)?;
            if time - clip.start < MIN_CLIP || clip.end - time < MIN_CLIP {
                return Err(AppError::Refused(
                    "Move the playhead inside the clip to split it.".into(),
                ));
            }
            vec![command("split", &[seconds(time)?])]
        }
        EditOp::Delete { .. } => vec![command("delete", &[])],
        EditOp::Duplicate { .. } => vec![command("duplicate", &[])],
        EditOp::Place { start, track, .. } => {
            let start = finite(start)?.max(0.0);
            if !(0..=MAX_TRACK).contains(&track) {
                return Err(AppError::InvalidInput(format!("A track is numbered 0 to {MAX_TRACK}.")));
            }
            let mut steps = Vec::new();
            if track != clip.track {
                // The engine moves a clip to any track without looking at what
                // is already there, so the look is taken here.
                let end = start + (clip.end - clip.start);
                let taken = others.iter().find(|other| {
                    other.track == track
                        && other.reference != clip.reference
                        && start < other.end - 1e-6
                        && end > other.start + 1e-6
                });
                if let Some(taken) = taken {
                    return Err(AppError::Refused(format!(
                        "{} would overlap {} on track {track}.",
                        clip.reference, taken.reference
                    )));
                }
                steps.push(command("set", &[format!("track={track}")]));
            }
            if (start - clip.start).abs() >= 0.001 {
                steps.push(command("move", &[seconds(start)?]));
            }
            if steps.is_empty() {
                return Err(AppError::Refused("The clip is already there.".into()));
            }
            steps
        }
        EditOp::SetVolume { volume, .. } => {
            let volume = finite(volume)?;
            if clip.volume.is_none() {
                return Err(AppError::Refused("This clip has no sound to adjust.".into()));
            }
            if !(0.0..=MAX_VOLUME).contains(&volume) {
                return Err(AppError::InvalidInput("Volume goes from 0 to 200%.".into()));
            }
            vec![command("set", &[format!("volume={}", seconds(volume)?)])]
        }
    };
    Ok(steps)
}

fn too_short() -> AppError {
    AppError::Refused("That would leave the clip too short.".into())
}

fn finite(value: f64) -> AppResult<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(AppError::InvalidInput("A time must be a number.".into()))
    }
}

/// A time as the engine's argument: never negative, to the millisecond.
fn seconds(value: f64) -> AppResult<String> {
    let text = format!("{:.3}", finite(value)?.max(0.0));
    Ok(text.trim_end_matches('0').trim_end_matches('.').to_owned())
}

/// A reference is passed as a command argument, so it must not look like an
/// option or carry control characters.
fn safe_reference(reference: &str) -> AppResult<String> {
    let safe = !reference.is_empty()
        && reference.len() <= 200
        && !reference.starts_with('-')
        && !reference.chars().any(char::is_control);
    if safe {
        Ok(reference.to_owned())
    } else {
        Err(AppError::InvalidInput("That clip cannot be addressed.".into()))
    }
}

/// Runs one edit and returns its receipt.
async fn run_edit(data_dir: &Path, project: &Path, args: &[String]) -> AppResult<Value> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = hyperframes::run(data_dir, &args, project, COMMAND_TIMEOUT).await?;
    let answer = answer_of(&output)?;
    if answer.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(AppError::Refused(refusal(&answer)));
    }
    answer
        .get("receipt")
        .filter(|receipt| receipt.is_object())
        .cloned()
        .ok_or_else(|| AppError::ToolFailed("The edit was made without an undo receipt.".into()))
}

/// The engine's JSON answer. A refusal is printed on the error stream.
fn answer_of(output: &std::process::Output) -> AppResult<Value> {
    parse_answer(&output.stdout)
        .or_else(|| parse_answer(&output.stderr))
        .ok_or_else(|| AppError::ToolFailed(shell_env::failure_text(output)))
}

fn parse_answer(stdout: &[u8]) -> Option<Value> {
    let text = String::from_utf8_lossy(stdout);
    serde_json::from_str(&text[text.find('{')?..]).ok()
}

/// The engine's reason for declining, as a sentence.
fn refusal(answer: &Value) -> String {
    let reason = answer.get("reason").and_then(Value::as_str).unwrap_or("The edit was not made");
    let mut sentence: String = reason.trim().trim_end_matches('.').to_owned();
    if let Some(first) = sentence.get(..1) {
        sentence = first.to_uppercase() + &sentence[1..];
    }
    sentence + "."
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn clip(media: bool) -> Clip {
        Clip {
            id: "vo".into(),
            reference: "#vo".into(),
            label: None,
            kind: if media { "audio" } else { "graphics" }.into(),
            track: 2,
            start: 2.0,
            end: 6.0,
            src: None,
            volume: media.then_some(1.0),
            media,
            editable: true,
        }
    }

    fn args(op: EditOp, clip: &Clip) -> Vec<String> {
        plan(&op, clip, &[]).unwrap().iter().map(|step| step.join(" ")).collect()
    }

    #[test]
    fn rows_become_clips_in_time_order() {
        let json = json!({ "timeline": { "duration": 12.5, "tracks": [
            { "kind": "audio", "rows": [
                { "id": "bed", "ref": "#bed", "kind": "audio", "trackKind": "audio", "trackIndex": 7,
                  "absStart": 3.0, "absEnd": 9.0, "src": "a.wav", "nested": false, "label": " " } ] },
            { "kind": "graphics", "rows": [
                { "id": "title", "ref": "#title", "kind": "div", "trackKind": "graphics", "trackIndex": 0,
                  "absStart": 0.0, "absEnd": 3.0, "src": null, "nested": true, "label": "Title" },
                { "id": "broken" } ] },
        ]}});
        let timeline = parse_timeline(&json).unwrap();
        assert_eq!(timeline.duration, 12.5);
        let ids: Vec<_> = timeline.clips.iter().map(|clip| clip.id.as_str()).collect();
        assert_eq!(ids, ["title", "bed"]);
        assert!(!timeline.clips[0].editable && timeline.clips[0].label.as_deref() == Some("Title"));
        assert!(timeline.clips[1].media && timeline.clips[1].editable && timeline.clips[1].label.is_none());
    }

    #[test]
    fn simple_edits_are_one_command() {
        let c = clip(false);
        assert_eq!(args(EditOp::Move { clip: "#vo".into(), start: 1.23456 }, &c), ["timeline move #vo 1.235 --json"]);
        assert_eq!(args(EditOp::Move { clip: "#vo".into(), start: -4.0 }, &c), ["timeline move #vo 0 --json"]);
        assert_eq!(args(EditOp::TrimEnd { clip: "#vo".into(), end: 5.0 }, &c), ["timeline trim #vo --end 5 --json"]);
        assert_eq!(args(EditOp::Split { clip: "#vo".into(), time: 4.5 }, &c), ["timeline split #vo 4.5 --json"]);
        assert_eq!(args(EditOp::Delete { clip: "#vo".into() }, &c), ["timeline delete #vo --json"]);
        assert_eq!(args(EditOp::Duplicate { clip: "#vo".into() }, &c), ["timeline duplicate #vo --json"]);
    }

    #[test]
    fn trimming_the_start_keeps_the_end() {
        assert_eq!(
            args(EditOp::TrimStart { clip: "#vo".into(), start: 3.0 }, &clip(false)),
            ["timeline trim #vo --start 3 --duration 3 --json"]
        );
        assert_eq!(
            args(EditOp::TrimStart { clip: "#vo".into(), start: 1.0 }, &clip(false)),
            ["timeline trim #vo --start 1 --duration 5 --json"]
        );
    }

    #[test]
    fn trimming_the_start_of_media_cuts_it() {
        assert_eq!(
            args(EditOp::TrimStart { clip: "#vo".into(), start: 3.0 }, &clip(true)),
            ["timeline split #vo 3 --json", "timeline delete #vo --json"]
        );
        let outward = plan(&EditOp::TrimStart { clip: "#vo".into(), start: 1.0 }, &clip(true), &[]);
        assert_eq!(outward.unwrap_err().code(), "refused");
    }

    #[test]
    fn edits_that_leave_nothing_are_refused() {
        let c = clip(false);
        for op in [
            EditOp::TrimEnd { clip: "#vo".into(), end: 2.01 },
            EditOp::TrimStart { clip: "#vo".into(), start: 5.99 },
            EditOp::Split { clip: "#vo".into(), time: 2.0 },
            EditOp::Split { clip: "#vo".into(), time: 9.0 },
        ] {
            assert_eq!(plan(&op, &c, &[]).unwrap_err().code(), "refused");
        }
        let nan = plan(&EditOp::Move { clip: "#vo".into(), start: f64::NAN }, &c, &[]);
        assert_eq!(nan.unwrap_err().code(), "invalid_input");
    }

    #[test]
    fn nested_clips_and_odd_references_are_not_edited() {
        let mut nested = clip(false);
        nested.editable = false;
        assert_eq!(plan(&EditOp::Delete { clip: "#vo".into() }, &nested, &[]).unwrap_err().code(), "refused");

        let mut option = clip(false);
        option.reference = "--dir=/".into();
        assert_eq!(plan(&EditOp::Delete { clip: "--dir=/".into() }, &option, &[]).unwrap_err().code(), "invalid_input");
    }

    #[test]
    fn placing_a_clip_changes_its_track_then_its_time() {
        let c = clip(false); // track 2, 2 s to 6 s
        let place = |start: f64, track: i64| EditOp::Place { clip: "#vo".into(), start, track };
        assert_eq!(args(place(2.0, 5), &c), ["timeline set #vo track=5 --json"]);
        assert_eq!(args(place(3.5, 2), &c), ["timeline move #vo 3.5 --json"]);
        assert_eq!(args(place(3.5, 5), &c), ["timeline set #vo track=5 --json", "timeline move #vo 3.5 --json"]);
        assert_eq!(plan(&place(2.0, 2), &c, &[]).unwrap_err().code(), "refused");
        assert_eq!(plan(&place(2.0, -1), &c, &[]).unwrap_err().code(), "invalid_input");
        assert_eq!(plan(&place(2.0, 100), &c, &[]).unwrap_err().code(), "invalid_input");
    }

    #[test]
    fn a_clip_is_not_placed_on_top_of_another() {
        let c = clip(false); // track 2, 2 s to 6 s
        let mut neighbour = clip(false);
        neighbour.reference = "#other".into();
        neighbour.track = 5;
        neighbour.start = 6.0;
        neighbour.end = 9.0;
        let others = [c.clone(), neighbour];
        let place = |start: f64| EditOp::Place { clip: "#vo".into(), start, track: 5 };
        // Touching the neighbour's start is fine; reaching into it is not.
        assert!(plan(&place(2.0), &c, &others).is_ok());
        let overlap = plan(&place(3.0), &c, &others).unwrap_err();
        assert_eq!(overlap.code(), "refused");
        assert_eq!(overlap.to_string(), "#vo would overlap #other on track 5.");
    }

    #[test]
    fn volume_is_for_clips_with_sound_and_stays_in_range() {
        let volume = |volume: f64| EditOp::SetVolume { clip: "#vo".into(), volume };
        assert_eq!(args(volume(0.5), &clip(true)), ["timeline set #vo volume=0.5 --json"]);
        assert_eq!(args(volume(0.0), &clip(true)), ["timeline set #vo volume=0 --json"]);
        assert_eq!(plan(&volume(2.5), &clip(true), &[]).unwrap_err().code(), "invalid_input");
        assert_eq!(plan(&volume(-0.1), &clip(true), &[]).unwrap_err().code(), "invalid_input");
        assert_eq!(plan(&volume(0.5), &clip(false), &[]).unwrap_err().code(), "refused");
    }

    #[test]
    fn a_refusal_reads_as_a_sentence() {
        let answer = json!({ "ok": false, "reason": "#vo1 would overlap #vo2 at 4-7.755" });
        assert_eq!(refusal(&answer), "#vo1 would overlap #vo2 at 4-7.755.");
        assert_eq!(refusal(&json!({ "ok": false, "reason": "file changed." })), "File changed.");
        assert_eq!(refusal(&json!({})), "The edit was not made.");
    }

    #[test]
    fn a_backup_the_engine_discarded_is_put_back_from_the_apps_copy() {
        let data = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path();
        let folder = project.join(".hyperframes/backup");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("b1"), "before the edit").unwrap();
        let receipt = json!({ "backupPath": ".hyperframes/backup/b1" });
        let store = BackupStore { dir: data.path().join("undo") };

        assert!(store.has(project, &receipt));
        store.keep(project, &receipt);
        std::fs::remove_file(folder.join("b1")).unwrap(); // the engine prunes it
        assert!(store.has(project, &receipt), "the app still has its copy");
        assert!(store.restore(project, &receipt));
        assert_eq!(std::fs::read_to_string(folder.join("b1")).unwrap(), "before the edit");

        std::fs::remove_file(folder.join("b1")).unwrap();
        store.forget(project);
        assert!(!store.has(project, &receipt));
        assert!(!store.restore(project, &receipt));
    }

    #[test]
    fn a_receipt_pointing_outside_the_backup_folder_is_not_followed() {
        let project = Path::new("/films/a");
        for path in ["../../etc/passwd", "/etc/passwd", "index.html", ".hyperframes/backup/../../x"] {
            assert_eq!(in_project(project, &json!({ "backupPath": path })), None, "{path}");
        }
        assert_eq!(
            in_project(project, &json!({ "backupPath": ".hyperframes/backup/b1" })),
            Some(project.join(".hyperframes/backup/b1"))
        );
        assert_eq!(in_project(project, &json!({})), None);
    }

    #[test]
    fn history_flags_follow_the_stacks() {
        let empty = Timeline { duration: 0.0, clips: vec![] };
        let none = state(empty.clone(), None);
        assert!(!none.can_undo && !none.can_redo);
        let history = History { undo: vec![vec![json!({})]], redo: vec![] };
        let some = state(empty, Some(&history));
        assert!(some.can_undo && !some.can_redo);
    }
}
