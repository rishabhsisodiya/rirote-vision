//! Jobs: voiceover, music, transcription, background removal and export.
//!
//! Each is one command that can take from a second to many minutes,
//! so they wait in a queue and run one at a time. The window is sent the
//! whole list whenever anything about it changes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Notify;

use crate::error::{AppError, AppResult};
use crate::media::{self, MediaKind};
use crate::render::{self, ExportOptions};
use crate::{hyperframes, log, music, shell_env};

const JOB_TIMEOUT: Duration = Duration::from_secs(45 * 60);
/// A long film at delivery quality can take hours.
const EXPORT_TIMEOUT: Duration = Duration::from_secs(8 * 60 * 60);
const MAX_TEXT: usize = 5_000;
/// Finished jobs kept in the list before the oldest are dropped.
const MAX_FINISHED: usize = 20;
const MAX_PROGRESS: usize = 120;
/// The engine marks machine-readable render progress with this.
const PROGRESS_MARK: &str = "@hf-progress ";

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum JobSpec {
    /// Speak `text` in `voice` into a new sound file.
    Voiceover { text: String, voice: String },
    /// Write captions and a word-timed transcript for a sound or video file.
    Transcribe { source: String },
    /// Cut the subject out of an image or video.
    RemoveBackground { source: String },
    /// Render the composition to a video file.
    Export(ExportOptions),
    /// Synthesise an instrumental bed in a style, `seconds` long.
    #[serde(rename_all = "camelCase")]
    Music { style: String, seconds: f64, impact_at: Option<f64> },
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: u64,
    pub project_path: String,
    /// voiceover, music, transcribe, removeBackground or export.
    pub kind: &'static str,
    pub label: String,
    pub status: JobStatus,
    /// What the engine last said it was doing.
    pub progress: Option<String>,
    /// How far along it is, when the engine says.
    pub percent: Option<u8>,
    /// The file the job made: relative to the project when it is inside it.
    pub output: Option<String>,
    /// The same file's full path.
    pub file: Option<String>,
    /// Something worth knowing about the file that was made.
    pub note: Option<String>,
    pub error: Option<String>,
}

/// What a job's command is run with.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Program {
    /// The video engine.
    Engine,
    /// The app's music script, in the app's Python environment.
    Music,
}

/// The command behind a job, and the tidying around it.
#[derive(Debug)]
struct Plan {
    program: Program,
    args: Vec<String>,
    project: PathBuf,
    /// The file that exists when the job has worked.
    output: PathBuf,
    /// A file the engine writes that belongs somewhere else afterwards.
    relocate: Option<(PathBuf, PathBuf)>,
    /// Scratch files to remove when the job ends.
    scratch: Option<PathBuf>,
    timeout: Duration,
    /// The output is a rendered film, to be looked at once it is written.
    film: bool,
}

pub type Listener = Arc<dyn Fn(Vec<Job>) + Send + Sync>;

#[derive(Default)]
struct State {
    jobs: Vec<Job>,
    plans: HashMap<u64, Plan>,
    /// The running job and its process group.
    running: Option<(u64, i32)>,
    /// The running job was asked to stop.
    cancelling: bool,
    next_id: u64,
}

pub struct JobQueue {
    data_dir: PathBuf,
    state: Mutex<State>,
    wake: Notify,
    listener: Mutex<Option<Listener>>,
}

impl JobQueue {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            state: Mutex::new(State { next_id: 1, ..State::default() }),
            wake: Notify::new(),
            listener: Mutex::new(None),
        }
    }

    /// Who is told when the list changes.
    pub fn listen(&self, listener: Listener) {
        *lock(&self.listener) = Some(listener);
    }

    pub fn list(&self, project: &Path) -> Vec<Job> {
        let project = project.to_string_lossy();
        lock(&self.state).jobs.iter().filter(|job| job.project_path == project).cloned().collect()
    }

    /// Adds a job to the queue. It is refused here if it could never run.
    pub fn add(&self, project: &Path, spec: JobSpec) -> AppResult<Job> {
        let job = {
            let mut state = lock(&self.state);
            let id = state.next_id;
            let (kind, label, plan) = plan(&spec, project, &self.data_dir, id)?;
            state.next_id += 1;
            let job = Job {
                id,
                project_path: project.to_string_lossy().into_owned(),
                kind,
                label,
                status: JobStatus::Queued,
                progress: None,
                percent: None,
                output: None,
                file: None,
                note: None,
                error: None,
            };
            state.jobs.push(job.clone());
            state.plans.insert(id, plan);
            job
        };
        self.announce();
        self.wake.notify_one();
        Ok(job)
    }

    /// Takes a waiting job off the queue, or stops the one that is running.
    pub fn cancel(&self, id: u64) {
        {
            let mut state = lock(&self.state);
            if let Some((running, pid)) = state.running {
                if running == id {
                    state.cancelling = true;
                    // SAFETY: `pid` is the process group this queue created
                    // for the job; the signal reaches nothing else.
                    unsafe { libc::killpg(pid, libc::SIGTERM) };
                }
            }
            if let Some(job) = state.jobs.iter_mut().find(|job| job.id == id && job.status == JobStatus::Queued) {
                job.status = JobStatus::Cancelled;
                if let Some(plan) = state.plans.remove(&id) {
                    tidy(&plan);
                }
            }
        }
        self.announce();
    }

    /// Removes finished jobs from a project's list.
    pub fn clear_finished(&self, project: &Path) {
        let project = project.to_string_lossy().into_owned();
        lock(&self.state).jobs.retain(|job| job.project_path != project || active(job.status));
        self.announce();
    }

    /// Stops everything. For when the app quits.
    pub fn stop_all(&self) {
        let ids: Vec<u64> = lock(&self.state).jobs.iter().filter(|job| active(job.status)).map(|job| job.id).collect();
        for id in ids {
            self.cancel(id);
        }
    }

    /// Runs queued jobs, one after another, for as long as the app lives.
    pub async fn work(self: Arc<Self>) {
        loop {
            let next = {
                let mut state = lock(&self.state);
                let id = state.jobs.iter().find(|job| job.status == JobStatus::Queued).map(|job| job.id);
                id.and_then(|id| state.plans.remove(&id).map(|plan| (id, plan)))
            };
            match next {
                Some((id, plan)) => self.run(id, plan).await,
                None => self.wake.notified().await,
            }
        }
    }

    async fn run(&self, id: u64, plan: Plan) {
        self.update(id, |job| job.status = JobStatus::Running);
        let outcome = self.execute(id, &plan).await;
        let cancelled = {
            let mut state = lock(&self.state);
            state.running = None;
            std::mem::take(&mut state.cancelling)
        };
        tidy(&plan);

        let (status, note, error) = match outcome {
            _ if cancelled => {
                let _ = std::fs::remove_file(&plan.output);
                (JobStatus::Cancelled, None, None)
            }
            Ok(note) => (JobStatus::Done, note, None),
            Err(error) => (JobStatus::Failed, None, Some(error.to_string())),
        };
        log::write(&self.data_dir, "jobs", &format!("job {id} {status:?}"));
        self.update(id, |job| {
            job.status = status;
            job.progress = None;
            job.percent = None;
            job.note = note;
            job.error = error;
            if status == JobStatus::Done {
                let inside = plan.output.strip_prefix(&plan.project).unwrap_or(&plan.output);
                job.output = Some(inside.to_string_lossy().into_owned());
                job.file = Some(plan.output.to_string_lossy().into_owned());
            }
        });
        self.trim();
    }

    /// Runs the job's command. Resolves to a note about the file it made.
    async fn execute(&self, id: u64, plan: &Plan) -> AppResult<Option<String>> {
        if let Some(parent) = plan.output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut command = match plan.program {
            Program::Engine => {
                let args: Vec<&str> = plan.args.iter().map(String::as_str).collect();
                hyperframes::command(&self.data_dir, &args, &plan.project)?
            }
            Program::Music => music::command(&self.data_dir, &plan.args, &plan.project)?,
        };
        // Its own process group, so cancelling stops the tools it starts too.
        command.process_group(0);
        let mut child = command
            .spawn()
            .map_err(|error| AppError::ToolFailed(format!("The job could not start: {error}")))?;
        if let Some(pid) = child.id().and_then(|pid| i32::try_from(pid).ok()) {
            lock(&self.state).running = Some((id, pid));
        }

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        // Both streams are read as they come: progress may be on either, and
        // an unread pipe would stall the engine.
        let answer = async {
            let mut text = String::new();
            if let Some(stdout) = stdout {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    self.note_progress(id, &line, plan.film);
                    if !line.starts_with(PROGRESS_MARK) {
                        text.push_str(&line);
                        text.push('\n');
                    }
                }
            }
            text
        };
        let follow = async {
            let mut last = String::new();
            if let Some(stderr) = stderr {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(said) = self.note_progress(id, &line, plan.film) {
                        last = said;
                    }
                }
            }
            last
        };
        let both = async {
            let (stdout, last) = tokio::join!(answer, follow);
            (child.wait().await, stdout, last)
        };
        let (status, stdout, last) = match tokio::time::timeout(plan.timeout, both).await {
            Ok(done) => done,
            Err(_) => return Err(AppError::Timeout("The job took too long and was stopped.".into())),
        };

        let reported = failure_in(&stdout);
        let succeeded = status.map(|status| status.success()).unwrap_or(false) && reported.is_none();
        if succeeded && plan.output.is_file() {
            if let Some((from, to)) = &plan.relocate {
                let _ = std::fs::rename(from, to);
            }
            if !plan.film {
                return Ok(None);
            }
            // A render that "worked" must also have left a playable film.
            let expected = crate::timeline::duration(&self.data_dir, &plan.project).await;
            return render::inspect(&plan.output, expected).await.map(Some);
        }
        let why = reported
            .or_else(|| stdout.lines().rev().find(|line| !line.trim().is_empty() && !line.trim_start().starts_with('{')).map(str::to_owned))
            .or_else(|| (!last.is_empty()).then_some(last))
            .unwrap_or_else(|| "The engine did not produce the file.".into());
        Err(AppError::ToolFailed(why))
    }

    /// Shows a line of the engine's output as the job's progress, if it is
    /// progress and says something new. Returns what it said. A render
    /// reports its progress in marked lines; the rest of what it prints is
    /// a terminal summary and is not shown.
    fn note_progress(&self, id: u64, line: &str, marked_only: bool) -> Option<String> {
        if marked_only && !line.trim_start().starts_with(PROGRESS_MARK) {
            return None;
        }
        let (said, percent) = progress_of(line)?;
        let mut changed = false;
        if let Some(job) = lock(&self.state).jobs.iter_mut().find(|job| job.id == id) {
            changed = job.progress.as_deref() != Some(said.as_str()) || (percent.is_some() && job.percent != percent);
            job.progress = Some(said.clone());
            if percent.is_some() {
                job.percent = percent;
            }
        }
        if changed {
            self.announce();
        }
        Some(said)
    }

    fn update(&self, id: u64, change: impl FnOnce(&mut Job)) {
        if let Some(job) = lock(&self.state).jobs.iter_mut().find(|job| job.id == id) {
            change(job);
        }
        self.announce();
    }

    /// Keeps the list from growing without end.
    fn trim(&self) {
        let mut state = lock(&self.state);
        let finished = state.jobs.iter().filter(|job| !active(job.status)).count();
        let mut extra = finished.saturating_sub(MAX_FINISHED);
        state.jobs.retain(|job| {
            let drop = extra > 0 && !active(job.status);
            if drop {
                extra -= 1;
            }
            !drop
        });
    }

    fn announce(&self) {
        let listener = lock(&self.listener).clone();
        if let Some(listener) = listener {
            listener(lock(&self.state).jobs.clone());
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn active(status: JobStatus) -> bool {
    matches!(status, JobStatus::Queued | JobStatus::Running)
}

fn tidy(plan: &Plan) {
    if let Some(scratch) = &plan.scratch {
        let _ = std::fs::remove_dir_all(scratch);
    }
}

/// Checks a request and works out the command that carries it out.
fn plan(spec: &JobSpec, project: &Path, data_dir: &Path, id: u64) -> AppResult<(&'static str, String, Plan)> {
    let scratch = data_dir.join("tmp").join(format!("job-{}-{id}", std::process::id()));
    let text = |path: &Path| path.to_string_lossy().into_owned();
    match spec {
        JobSpec::Voiceover { text: script, voice } => {
            let script = script.trim();
            if script.is_empty() {
                return Err(AppError::InvalidInput("Type the words to speak first.".into()));
            }
            if script.chars().count() > MAX_TEXT {
                return Err(AppError::InvalidInput(format!(
                    "A voiceover can be at most {MAX_TEXT} characters. Split it into parts."
                )));
            }
            if !valid_voice(voice) {
                return Err(AppError::InvalidInput("That is not a voice the engine has.".into()));
            }
            std::fs::create_dir_all(&scratch)?;
            let script_file = scratch.join("script.txt");
            std::fs::write(&script_file, script)?;
            let folder = media::assets_dir(project).join("voiceover");
            let output = media::free_name(&folder, Path::new(&format!("{}.wav", slug(script))));
            let args = vec!["tts".into(), text(&script_file), "-o".into(), text(&output), "--voice".into(), voice.clone(), "--json".into()];
            let label = format!("Voiceover: {}", excerpt(script));
            Ok(("voiceover", label, Plan { program: Program::Engine, args, project: project.to_path_buf(), output, relocate: None, scratch: Some(scratch), timeout: JOB_TIMEOUT, film: false }))
        }
        JobSpec::Transcribe { source } => {
            let file = source_file(project, source, &[MediaKind::Audio, MediaKind::Video], "Only sound and video files can be transcribed.")?;
            let folder = file.parent().unwrap_or(project);
            let stem = file.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            let output = media::free_name(folder, Path::new(&format!("{stem}.srt")));
            let transcript = folder.join(format!("{}.transcript.json", output.file_stem().unwrap_or_default().to_string_lossy()));
            std::fs::create_dir_all(&scratch)?;
            // The engine names its transcript `transcript.json`; pointing it at
            // a scratch folder keeps it from overwriting one in the project.
            let args = vec![
                "transcribe".into(), text(&file), "--json".into(), "--to".into(), "srt".into(),
                "-o".into(), text(&output), "--dir".into(), text(&scratch),
            ];
            let relocate = Some((scratch.join("transcript.json"), transcript));
            Ok(("transcribe", format!("Captions: {}", name_of(&file)), Plan { program: Program::Engine, args, project: project.to_path_buf(), output, relocate, scratch: Some(scratch), timeout: JOB_TIMEOUT, film: false }))
        }
        JobSpec::RemoveBackground { source } => {
            let file = source_file(project, source, &[MediaKind::Image, MediaKind::Video], "Only images and videos can have their background removed.")?;
            let extension = file.extension().and_then(|ext| ext.to_str()).unwrap_or("").to_ascii_lowercase();
            if matches!(extension.as_str(), "svg" | "gif" | "avif") {
                return Err(AppError::InvalidInput("That image format cannot have its background removed. Use PNG, JPEG or WebP.".into()));
            }
            let folder = file.parent().unwrap_or(project);
            let stem = file.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            let made = if MediaKind::of(&file) == Some(MediaKind::Video) { "webm" } else { "png" };
            let output = media::free_name(folder, Path::new(&format!("{stem}-cutout.{made}")));
            let args = vec!["remove-background".into(), text(&file), "-o".into(), text(&output), "--json".into()];
            Ok(("removeBackground", format!("Cut-out: {}", name_of(&file)), Plan { program: Program::Engine, args, project: project.to_path_buf(), output, relocate: None, scratch: None, timeout: JOB_TIMEOUT, film: false }))
        }
        JobSpec::Export(options) => {
            let (output, args) = render::plan(options, project)?;
            let label = format!("Export: {}", name_of(&output));
            Ok(("export", label, Plan { program: Program::Engine, args, project: project.to_path_buf(), output, relocate: None, scratch: None, timeout: EXPORT_TIMEOUT, film: true }))
        }
        JobSpec::Music { style, seconds, impact_at } => {
            let music::MusicPlan { output, args, label } = music::plan(style, *seconds, *impact_at, project)?;
            Ok(("music", label, Plan { program: Program::Music, args, project: project.to_path_buf(), output, relocate: None, scratch: None, timeout: JOB_TIMEOUT, film: false }))
        }
    }
}

/// The existing project file a job works on, checked for kind.
fn source_file(project: &Path, relative: &str, kinds: &[MediaKind], refusal: &str) -> AppResult<PathBuf> {
    let file = media::resolve(project, relative)?;
    if !file.is_file() {
        return Err(AppError::NotFound(format!("{relative} is no longer in the project.")));
    }
    match MediaKind::of(&file) {
        Some(kind) if kinds.contains(&kind) => Ok(file),
        _ => Err(AppError::InvalidInput(refusal.into())),
    }
}

fn name_of(path: &Path) -> String {
    path.file_name().unwrap_or_default().to_string_lossy().into_owned()
}

/// Voice ids look like `af_heart`: nothing that could be read as an option.
fn valid_voice(voice: &str) -> bool {
    let mut parts = voice.split('_');
    let ok = |part: Option<&str>, max: usize| {
        part.is_some_and(|part| !part.is_empty() && part.len() <= max && part.bytes().all(|b| b.is_ascii_lowercase()))
    };
    ok(parts.next(), 2) && ok(parts.next(), 24) && parts.next().is_none()
}

/// A file name from the first words of a script.
fn slug(script: &str) -> String {
    let mut slug = String::new();
    for ch in script.chars() {
        if slug.len() >= 32 {
            break;
        }
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() { "voiceover".into() } else { slug.to_owned() }
}

fn excerpt(script: &str) -> String {
    let line: String = script.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= 40 {
        line
    } else {
        format!("{}…", line.chars().take(40).collect::<String>().trim_end())
    }
}

/// What a line of the engine's output says about progress, if anything
/// worth showing, and how far along that is.
fn progress_of(line: &str) -> Option<(String, Option<u8>)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    if let Some(report) = line.strip_prefix(PROGRESS_MARK) {
        let json: Value = serde_json::from_str(report).ok()?;
        let said = render::stage(json["code"].as_str().unwrap_or(""), json["done"].as_u64(), json["total"].as_u64());
        let percent = json["pct"].as_u64().map(|pct| pct.min(100) as u8);
        return Some((said, percent));
    }
    if line.starts_with('{') {
        let json: Value = serde_json::from_str(line).ok()?;
        if json["type"] != "progress" {
            return None;
        }
        let words: Vec<&str> = ["phase", "status", "message"].iter().filter_map(|key| json[*key].as_str()).collect();
        return (!words.is_empty()).then(|| (words.join(": "), None));
    }
    // Log lines, progress bars and their colour codes are for a terminal.
    if line.starts_with('[') || line.contains('█') || line.contains('░') {
        return None;
    }
    let plain: String = strip_ansi(line).chars().filter(|ch| ch.is_ascii_graphic() || *ch == ' ' || ch.is_alphanumeric()).collect();
    let plain = plain.trim();
    (!plain.is_empty()).then(|| (plain.chars().take(MAX_PROGRESS).collect(), None))
}

fn strip_ansi(text: &str) -> String {
    let mut plain = String::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            // Skip to the letter that ends the code.
            for code in chars.by_ref() {
                if code.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            plain.push(ch);
        }
    }
    plain
}

/// The reason the engine gave for failing, from its JSON answer.
fn failure_in(stdout: &str) -> Option<String> {
    let answer: Value = stdout.lines().rev().find_map(|line| serde_json::from_str(line.trim()).ok())?;
    if answer["ok"].as_bool() != Some(false) {
        return None;
    }
    let reason = answer["error"].as_str().or(answer["reason"].as_str()).unwrap_or("The engine reported a failure.");
    Some(shell_env::first_line(reason))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_with(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for file in files {
            let path = dir.path().join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"x").unwrap();
        }
        dir
    }

    #[test]
    fn a_voiceover_is_named_after_its_words_and_never_overwrites() {
        let data = tempfile::tempdir().unwrap();
        let project = project_with(&["assets/voiceover/hello-world.wav"]);
        let spec = JobSpec::Voiceover { text: "  Hello, world!  ".into(), voice: "af_heart".into() };
        let (kind, label, plan) = plan(&spec, project.path(), data.path(), 7).unwrap();
        assert_eq!(kind, "voiceover");
        assert_eq!(label, "Voiceover: Hello, world!");
        assert_eq!(plan.output, project.path().join("assets/voiceover/hello-world-2.wav"));
        assert_eq!(plan.args[0], "tts");
        assert_eq!(std::fs::read_to_string(&plan.args[1]).unwrap(), "Hello, world!");
        assert_eq!(&plan.args[4..], ["--voice", "af_heart", "--json"]);
        tidy(&plan);
        assert!(!Path::new(&plan.args[1]).exists(), "the script file is scratch");
    }

    #[test]
    fn a_voiceover_needs_words_and_a_real_voice() {
        let data = tempfile::tempdir().unwrap();
        let project = project_with(&[]);
        let job = |text: &str, voice: &str| {
            plan(&JobSpec::Voiceover { text: text.into(), voice: voice.into() }, project.path(), data.path(), 1)
        };
        assert_eq!(job("   ", "af_heart").unwrap_err().code(), "invalid_input");
        assert_eq!(job(&"a".repeat(MAX_TEXT + 1), "af_heart").unwrap_err().code(), "invalid_input");
        for voice in ["", "--help", "af heart", "af_heart_x", "AF_heart", "af_"] {
            assert_eq!(job("hi", voice).unwrap_err().code(), "invalid_input", "{voice}");
        }
    }

    #[test]
    fn captions_sit_beside_the_file_and_the_transcript_is_steered_away() {
        let data = tempfile::tempdir().unwrap();
        let project = project_with(&["assets/talk.mp4", "assets/talk.srt", "assets/logo.png"]);
        let spec = JobSpec::Transcribe { source: "assets/talk.mp4".into() };
        let (kind, label, plan) = plan(&spec, project.path(), data.path(), 3).unwrap();
        assert_eq!((kind, label.as_str()), ("transcribe", "Captions: talk.mp4"));
        assert_eq!(plan.output, project.path().join("assets/talk-2.srt"));
        let (from, to) = plan.relocate.as_ref().unwrap();
        assert!(from.starts_with(data.path()) && from.ends_with("transcript.json"));
        assert_eq!(to, &project.path().join("assets/talk-2.transcript.json"));

        let wrong = plan_error(&JobSpec::Transcribe { source: "assets/logo.png".into() }, &project, &data);
        assert_eq!(wrong, "invalid_input");
        let gone = plan_error(&JobSpec::Transcribe { source: "assets/gone.mp4".into() }, &project, &data);
        assert_eq!(gone, "not_found");
        let outside = plan_error(&JobSpec::Transcribe { source: "../other/a.mp4".into() }, &project, &data);
        assert_eq!(outside, "invalid_input");
    }

    fn plan_error(spec: &JobSpec, project: &tempfile::TempDir, data: &tempfile::TempDir) -> &'static str {
        plan(spec, project.path(), data.path(), 1).unwrap_err().code()
    }

    #[test]
    fn a_cut_out_keeps_the_kind_of_its_source() {
        let data = tempfile::tempdir().unwrap();
        let project = project_with(&["assets/me.jpg", "assets/clip.mov", "assets/logo.svg", "assets/vo.wav"]);
        let cut = |source: &str| plan(&JobSpec::RemoveBackground { source: source.into() }, project.path(), data.path(), 1);
        assert_eq!(cut("assets/me.jpg").unwrap().2.output, project.path().join("assets/me-cutout.png"));
        assert_eq!(cut("assets/clip.mov").unwrap().2.output, project.path().join("assets/clip-cutout.webm"));
        assert_eq!(cut("assets/logo.svg").unwrap_err().code(), "invalid_input");
        assert_eq!(cut("assets/vo.wav").unwrap_err().code(), "invalid_input");
    }

    #[test]
    fn names_and_excerpts_are_tidy() {
        assert_eq!(slug("Rirote Vision — turns a sentence into a finished video."), "rirote-vision-turns-a-sentence-i");
        assert_eq!(slug("¡¡¡"), "voiceover");
        assert_eq!(excerpt("one\ntwo   three"), "one two three");
        assert!(excerpt(&"word ".repeat(30)).ends_with('…'));
    }

    #[test]
    fn progress_lines_are_read_whatever_their_form() {
        let said = |line: &str| progress_of(line).map(|(said, percent)| (said, percent));
        let json = r#"{"type":"progress","phase":"transcription","model":"small.en","status":"completed"}"#;
        assert_eq!(said(json), Some(("transcription: completed".into(), None)));
        assert_eq!(said(r#"{"type":"result","words":[]}"#), None);
        assert_eq!(said("\u{1b}[36m◇\u{1b}[39m  Downloading model 42%"), Some(("Downloading model 42%".into(), None)));
        assert_eq!(said("   "), None);

        let render = r#"@hf-progress {"code":"capture","done":120,"total":300,"pct":57}"#;
        assert_eq!(said(render), Some(("Capturing frame 120 of 300".into(), Some(57))));
        assert_eq!(said(r#"@hf-progress {"code":"compile","pct":5}"#), Some(("Preparing the composition".into(), Some(5))));
        assert_eq!(said("  ██████░░░░░░░░░  25%  Starting frame capture"), None);
        assert_eq!(said(r#"[INFO] [Render] Pipeline started {"fps":30}"#), None);
    }

    #[test]
    fn a_music_bed_runs_the_apps_script_not_the_engine() {
        let data = tempfile::tempdir().unwrap();
        let project = project_with(&[]);
        let spec: JobSpec = serde_json::from_value(serde_json::json!({
            "type": "music", "style": "cinematic", "seconds": 20, "impactAt": 12
        }))
        .unwrap();
        let (kind, label, plan) = plan(&spec, project.path(), data.path(), 1).unwrap();
        assert_eq!((kind, label.as_str()), ("music", "Music: Cinematic build, 20 s"));
        assert_eq!(plan.program, Program::Music);
        assert_eq!(plan.output, project.path().join("assets/music/cinematic-20s.wav"));
        assert_eq!(plan.args[3], "12");

        let odd: JobSpec = serde_json::from_value(serde_json::json!({ "type": "music", "style": "jazz", "seconds": 20, "impactAt": null })).unwrap();
        assert_eq!(plan_error(&odd, &project, &data), "invalid_input");
    }

    #[test]
    fn an_export_is_a_job_like_the_others() {
        let data = tempfile::tempdir().unwrap();
        let project = project_with(&[]);
        let spec: JobSpec = serde_json::from_value(serde_json::json!({
            "type": "export", "format": "mp4", "quality": "draft", "fps": null, "resolution": null, "name": "Cut 1", "folder": null
        }))
        .unwrap();
        let (kind, label, plan) = plan(&spec, project.path(), data.path(), 1).unwrap();
        assert_eq!((kind, label.as_str()), ("export", "Export: Cut 1.mp4"));
        assert!(plan.film && plan.timeout > JOB_TIMEOUT);
        assert_eq!(plan.program, Program::Engine);
        assert_eq!(plan.output, project.path().join("renders/Cut 1.mp4"));
        assert_eq!(&plan.args[3..], ["-q", "draft", "--format", "mp4"]);
    }

    #[test]
    fn a_reported_failure_is_found_in_the_answer() {
        assert_eq!(failure_in("{\"ok\":false,\"error\":\"No speech found\\nmore\"}\n").as_deref(), Some("No speech found"));
        assert_eq!(failure_in("{\"ok\":true,\"outputPath\":\"x\"}"), None);
        assert_eq!(failure_in("File not found: a.wav"), None);
    }

    #[tokio::test]
    async fn a_waiting_job_can_be_cancelled_and_finished_ones_cleared() {
        let data = tempfile::tempdir().unwrap();
        let project = project_with(&["assets/talk.mp4"]);
        let queue = JobQueue::new(data.path());
        let seen: Arc<Mutex<usize>> = Arc::default();
        let count = seen.clone();
        queue.listen(Arc::new(move |_| *count.lock().unwrap() += 1));

        let job = queue.add(project.path(), JobSpec::Transcribe { source: "assets/talk.mp4".into() }).unwrap();
        assert_eq!(job.status, JobStatus::Queued);
        assert_eq!(queue.list(project.path()).len(), 1);
        assert!(queue.list(Path::new("/another/project")).is_empty());

        queue.cancel(job.id);
        assert_eq!(queue.list(project.path())[0].status, JobStatus::Cancelled);
        queue.clear_finished(project.path());
        assert!(queue.list(project.path()).is_empty());
        assert!(*seen.lock().unwrap() >= 3, "every change is announced");
    }
}
