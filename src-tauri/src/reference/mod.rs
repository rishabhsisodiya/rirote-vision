//! References: videos studied for their structure, kept in an app-wide
//! library and never placed in a project. A port of reelforge's pipeline:
//! download, find the shots, take stills, transcribe, analyse.
//!
//! One study runs at a time, beside (not behind) the project jobs, so a study
//! never holds up an export.

mod analyze;
pub mod model;
mod run;
pub mod ytdlp;
mod video;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::Notify;

use crate::error::{AppError, AppResult};
use crate::settings::Settings;
use crate::{hyperframes, ids, log, shell_env, store};
use model::{Blueprint, Found, Reference, ReferenceMeta, Run, ShotList, StepState, Word};
use run::Cancel;

const TRANSCRIBE_LIMIT: Duration = Duration::from_secs(20 * 60);
const THUMB_LIMIT: Duration = Duration::from_secs(20);
const SEARCH_CACHE: Duration = Duration::from_secs(24 * 60 * 60);
const SEARCH_LIMIT: usize = 24;
const MAX_FILE_BYTES: u64 = 500 * 1024 * 1024;

/// Where a study's video comes from.
#[derive(Debug, Clone)]
enum Origin {
    Link(String),
    File(PathBuf),
}

#[derive(Default)]
struct Queue {
    waiting: VecDeque<(String, Origin)>,
    running: Option<String>,
    cancel: Cancel,
}

pub type Listener = Arc<dyn Fn(Vec<Reference>) + Send + Sync>;

pub struct ReferenceService {
    data_dir: PathBuf,
    root: PathBuf,
    queue: Mutex<Queue>,
    wake: Notify,
    listener: Mutex<Option<Listener>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceDetail {
    pub reference: Reference,
    pub blueprint: Option<Blueprint>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |time| time.as_secs())
}

fn slug(text: &str, max: usize) -> String {
    let mut out = String::new();
    for c in text.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed: String = out.trim_matches('-').chars().take(max).collect();
    let trimmed = trimmed.trim_matches('-').to_owned();
    if trimmed.is_empty() { "ref".into() } else { trimmed }
}

/// A folder name for a platform's video id. Ids like YouTube's differ only by
/// case, and macOS folder names ignore case, so a hash of the exact id is added.
fn case_safe_id(platform: &str, id: &str) -> String {
    format!("{}-{}-{:08x}", slug(platform, 12), slug(id, 40), ids::fnv1a(id.as_bytes()) as u32)
}

impl ReferenceService {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            root: data_dir.join("references"),
            queue: Mutex::new(Queue::default()),
            wake: Notify::new(),
            listener: Mutex::new(None),
        }
    }

    pub fn listen(&self, listener: Listener) {
        *self.listener.lock().unwrap_or_else(|p| p.into_inner()) = Some(listener);
    }

    fn dir(&self, id: &str) -> AppResult<PathBuf> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(AppError::InvalidInput("That is not a reference.".into()));
        }
        Ok(self.root.join(id))
    }

    fn announce(&self) {
        let listener = self.listener.lock().unwrap_or_else(|p| p.into_inner()).clone();
        if let Some(listener) = listener {
            listener(self.list());
        }
    }

    fn read(&self, dir: &Path) -> Option<Reference> {
        let meta: ReferenceMeta = serde_json::from_str(&std::fs::read_to_string(dir.join("meta.json")).ok()?).ok()?;
        let run: Run = store::read_or_default(&dir.join("run.json"));
        let queue = self.queue.lock().unwrap_or_else(|p| p.into_inner());
        let status = if queue.running.as_deref() == Some(meta.id.as_str()) {
            "running"
        } else if queue.waiting.iter().any(|(id, _)| *id == meta.id) {
            "queued"
        } else if run.finished() {
            "ready"
        } else {
            "failed"
        };
        drop(queue);
        let shots: Option<ShotList> = std::fs::read_to_string(dir.join("shots.json")).ok().and_then(|t| serde_json::from_str(&t).ok());
        let poster = shots
            .and_then(|list| list.shots.into_iter().find_map(|shot| shot.keyframe))
            .map(|still| dir.join(still))
            .filter(|path| path.is_file())
            .map(|path| path.to_string_lossy().into_owned());
        Some(Reference { meta, steps: run.steps, status, poster, folder: dir.to_string_lossy().into_owned() })
    }

    /// Newest first.
    pub fn list(&self) -> Vec<Reference> {
        let Ok(entries) = std::fs::read_dir(&self.root) else { return Vec::new() };
        let mut list: Vec<Reference> = entries.filter_map(Result::ok).filter_map(|entry| self.read(&entry.path())).collect();
        list.sort_by(|a, b| b.meta.added_at.cmp(&a.meta.added_at));
        list
    }

    pub fn get(&self, id: &str) -> AppResult<ReferenceDetail> {
        let dir = self.dir(id)?;
        let reference = self.read(&dir).ok_or_else(|| AppError::NotFound("That reference is not in the library.".into()))?;
        let blueprint = std::fs::read_to_string(dir.join("blueprint.json")).ok().and_then(|text| serde_json::from_str(&text).ok());
        Ok(ReferenceDetail { reference, blueprint })
    }

    /// Adds a link or a file to the library and queues its study. A reference
    /// already studied is returned as it is; one that failed is studied again
    /// from the step that failed.
    pub async fn import(&self, link: Option<String>, file: Option<String>, settings: &Settings) -> AppResult<Reference> {
        let (meta, origin) = match (link, file) {
            (Some(link), None) => {
                let link = ytdlp::check_link(&link)?;
                let info = ytdlp::info(&self.data_dir, &link, &Cancel::default()).await?;
                if let Some(seconds) = info.duration {
                    if seconds > f64::from(settings.reference_max_minutes) * 60.0 {
                        return Err(AppError::InvalidInput(format!(
                            "That video is {} minutes long; the limit is {}. Pick a shorter one, or raise the limit in Settings.",
                            (seconds / 60.0).ceil(),
                            settings.reference_max_minutes
                        )));
                    }
                }
                let id = case_safe_id(&info.platform, &info.id);
                let meta = ReferenceMeta {
                    id,
                    source: link.clone(),
                    platform: info.platform,
                    title: info.title,
                    creator: info.creator,
                    duration: info.duration,
                    width: None,
                    height: None,
                    added_at: now(),
                };
                (meta, Origin::Link(link))
            }
            (None, Some(file)) => {
                let path = PathBuf::from(&file);
                let size = std::fs::metadata(&path).map_err(|_| AppError::NotFound("That file is not there any more.".into()))?.len();
                if size > MAX_FILE_BYTES {
                    return Err(AppError::InvalidInput("That video is larger than 500 MB.".into()));
                }
                // The first 8 MB and the size tell copies of one file apart well enough.
                let mut head = Vec::new();
                {
                    use std::io::Read;
                    std::fs::File::open(&path)?.take(8 * 1024 * 1024).read_to_end(&mut head)?;
                }
                head.extend_from_slice(&size.to_le_bytes());
                let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let id = format!("file-{}-{:08x}", slug(&stem, 32), ids::fnv1a(&head) as u32);
                let meta = ReferenceMeta {
                    id,
                    source: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(file.clone()),
                    platform: "file".into(),
                    title: Some(stem),
                    creator: None,
                    duration: None,
                    width: None,
                    height: None,
                    added_at: now(),
                };
                (meta, Origin::File(path))
            }
            _ => return Err(AppError::InvalidInput("Give one link or one file.".into())),
        };

        let dir = self.dir(&meta.id)?;
        if let Some(existing) = self.read(&dir) {
            if existing.status != "failed" {
                return Ok(existing);
            }
        } else {
            std::fs::create_dir_all(&dir)?;
            store::write_atomic(&dir.join("meta.json"), &meta)?;
            store::write_atomic(&dir.join("run.json"), &Run::default())?;
        }
        self.enqueue(meta.id.clone(), origin);
        self.read(&dir).ok_or_else(|| AppError::Io("The reference could not be saved.".into()))
    }

    fn enqueue(&self, id: String, origin: Origin) {
        {
            let mut queue = self.queue.lock().unwrap_or_else(|p| p.into_inner());
            if queue.running.as_deref() == Some(id.as_str()) || queue.waiting.iter().any(|(queued, _)| *queued == id) {
                return;
            }
            queue.waiting.push_back((id, origin));
        }
        self.wake.notify_one();
        self.announce();
    }

    /// Studies a failed or stopped reference again, from the step that failed.
    pub fn retry(&self, id: &str) -> AppResult<Reference> {
        let dir = self.dir(id)?;
        let reference = self.read(&dir).ok_or_else(|| AppError::NotFound("That reference is not in the library.".into()))?;
        let origin = if reference.meta.platform == "file" {
            // The video was copied in; a missing copy means the file must be added again.
            match ytdlp::find_source(&dir) {
                Some(source) => Origin::File(source),
                None => return Err(AppError::NotFound("The video is gone. Add the file again.".into())),
            }
        } else {
            Origin::Link(reference.meta.source.clone())
        };
        self.enqueue(id.to_owned(), origin);
        self.read(&dir).ok_or_else(|| AppError::NotFound("That reference is not in the library.".into()))
    }

    /// Stops a running study, or takes a queued one off the queue.
    pub fn stop(&self, id: &str) {
        let mut queue = self.queue.lock().unwrap_or_else(|p| p.into_inner());
        queue.waiting.retain(|(queued, _)| queued != id);
        if queue.running.as_deref() == Some(id) {
            queue.cancel.raise();
        }
        drop(queue);
        self.announce();
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        let dir = self.dir(id)?;
        self.stop(id);
        if dir.is_dir() {
            std::fs::remove_dir_all(&dir)?;
        }
        self.announce();
        Ok(())
    }

    pub fn stop_all(&self) {
        let mut queue = self.queue.lock().unwrap_or_else(|p| p.into_inner());
        queue.waiting.clear();
        queue.cancel.raise();
    }

    /// The worker: one study at a time, for as long as the app runs.
    pub async fn work(self: Arc<Self>, settings: Arc<dyn Fn() -> Settings + Send + Sync>) {
        loop {
            let next = {
                let mut queue = self.queue.lock().unwrap_or_else(|p| p.into_inner());
                let next = queue.waiting.pop_front();
                if let Some((id, _)) = &next {
                    queue.running = Some(id.clone());
                    queue.cancel = Cancel::default();
                }
                next.map(|(id, origin)| (id, origin, queue.cancel.clone()))
            };
            let Some((id, origin, cancel)) = next else {
                self.wake.notified().await;
                continue;
            };
            self.announce();
            if let Ok(dir) = self.dir(&id) {
                let outcome = self.study(&dir, &origin, &settings(), &cancel).await;
                if let Err(error) = &outcome {
                    log::write(&self.data_dir, "reference", &format!("{id}: {error}"));
                }
            }
            self.queue.lock().unwrap_or_else(|p| p.into_inner()).running = None;
            self.announce();
        }
    }

    /// Runs each step whose result is not on disk yet; stops at the first failure.
    async fn study(&self, dir: &Path, origin: &Origin, settings: &Settings, cancel: &Cancel) -> AppResult<()> {
        let mut run: Run = store::read_or_default(&dir.join("run.json"));
        // Steps after a failure were skipped; they get another go.
        for step in &mut run.steps {
            if step.state != StepState::Done {
                step.state = StepState::Pending;
                step.error = None;
            }
        }
        store::write_atomic(&dir.join("run.json"), &run)?;
        let mut redo_after = false;
        for name in model::STEPS {
            let done = run.step(name).state == StepState::Done && !redo_after;
            if done {
                continue;
            }
            run.step(name).state = StepState::Running;
            store::write_atomic(&dir.join("run.json"), &run)?;
            self.announce();
            let started = Instant::now();
            let result = self.step(name, dir, origin, settings, cancel).await;
            let step = run.step(name);
            step.seconds = Some((started.elapsed().as_secs_f64() * 100.0).round() / 100.0);
            match result {
                Ok(()) => {
                    step.state = StepState::Done;
                    redo_after = true;
                }
                Err(error) => {
                    step.state = StepState::Failed;
                    step.error = Some(error.to_string());
                    for later in run.steps.iter_mut().filter(|later| later.state == StepState::Pending) {
                        later.state = StepState::Skipped;
                    }
                    store::write_atomic(&dir.join("run.json"), &run)?;
                    return Err(error);
                }
            }
            store::write_atomic(&dir.join("run.json"), &run)?;
            self.announce();
        }
        Ok(())
    }

    async fn step(&self, name: &str, dir: &Path, origin: &Origin, settings: &Settings, cancel: &Cancel) -> AppResult<()> {
        let source = || ytdlp::find_source(dir).ok_or_else(|| AppError::NotFound("The reference video is missing; study it again.".into()));
        match name {
            "download" => {
                for old in std::fs::read_dir(dir)?.filter_map(Result::ok) {
                    if old.file_name().to_string_lossy().starts_with("source.") {
                        let _ = std::fs::remove_file(old.path());
                    }
                }
                match origin {
                    Origin::Link(link) => {
                        ytdlp::download(&self.data_dir, link, dir, settings.reference_max_minutes, cancel).await?;
                    }
                    Origin::File(file) => {
                        let extension = file.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_else(|| "mp4".into());
                        let target = dir.join(format!("source.{extension}"));
                        if std::fs::hard_link(file, &target).is_err() {
                            std::fs::copy(file, &target)?;
                        }
                    }
                }
                let probe = video::probe(&source()?, cancel).await?;
                if probe.duration > f64::from(settings.reference_max_minutes) * 60.0 {
                    return Err(AppError::InvalidInput(format!(
                        "That video is longer than {} minutes. Raise the limit in Settings to study it.",
                        settings.reference_max_minutes
                    )));
                }
                let mut meta: ReferenceMeta = serde_json::from_str(&std::fs::read_to_string(dir.join("meta.json"))?)
                    .map_err(|error| AppError::Io(error.to_string()))?;
                meta.duration = Some(probe.duration);
                meta.width = Some(probe.width);
                meta.height = Some(probe.height);
                store::write_atomic(&dir.join("meta.json"), &meta)
            }
            "shots" => {
                let video = source()?;
                let probe = video::probe(&video, cancel).await?;
                let list = video::shots(&video, &probe, cancel).await?;
                store::write_atomic(&dir.join("shots.json"), &list)
            }
            "keyframes" => {
                let mut list: ShotList = read_json(&dir.join("shots.json"))?;
                video::keyframes(&source()?, dir, &mut list, cancel).await?;
                store::write_atomic(&dir.join("shots.json"), &list)
            }
            "transcribe" => self.transcribe(dir, &source()?, cancel).await,
            "analyze" => {
                let list: ShotList = read_json(&dir.join("shots.json"))?;
                let words: Vec<Word> = read_json(&dir.join("transcript.json"))?;
                let meta: ReferenceMeta = read_json(&dir.join("meta.json"))?;
                let language: Option<String> = std::fs::read_to_string(dir.join("language.txt")).ok().map(|l| l.trim().to_owned()).filter(|l| !l.is_empty());
                let (analysis, used) = analyze::analyze(dir, &list, &words, &settings.assistant_model, cancel).await?;
                store::write_atomic(&dir.join("blueprint.json"), &analyze::blueprint(&meta, &list, language, analysis, used))
            }
            _ => Ok(()),
        }
    }

    /// Words with times, from the engine's own Whisper: the multilingual model,
    /// so Hindi and Hinglish references are heard too. Never HeyGen's engine.
    async fn transcribe(&self, dir: &Path, video: &Path, cancel: &Cancel) -> AppResult<()> {
        let file = video.to_string_lossy().into_owned();
        let folder = dir.to_string_lossy().into_owned();
        let mut command = hyperframes::command(
            &self.data_dir,
            &["transcribe", &file, "--json", "--dir", &folder, "-m", "small", "-e", "whisper"],
            dir,
        )?;
        command.kill_on_drop(true);
        let output = run::run(command, TRANSCRIBE_LIMIT, "transcription", cancel).await?;
        let said = String::from_utf8_lossy(&output.stdout);
        let words: Vec<Word> = if output.status.success() {
            read_json(&dir.join("transcript.json")).unwrap_or_default()
        } else if said.contains("no audio") || shell_env::failure_text(&output).to_lowercase().contains("no audio") {
            Vec::new()
        } else {
            return Err(AppError::ToolFailed(format!("The words could not be transcribed.\n{}", shell_env::failure_text(&output))));
        };
        if let Some(language) = said
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line.trim()).ok())
            .find_map(|value| value.get("detectedLanguage").and_then(|l| l.as_str()).map(str::to_owned))
        {
            std::fs::write(dir.join("language.txt"), language)?;
        }
        // Kept in the app's own shape: text, start and end.
        store::write_atomic(&dir.join("transcript.json"), &words)
    }

    /// YouTube search, or an Instagram hashtag when Settings allow it. Results
    /// and their thumbnails are kept for a day.
    pub async fn search(&self, query: &str, source: &str, settings: &Settings) -> AppResult<Vec<Found>> {
        let cookies = match source {
            "instagram" if !settings.instagram_search => {
                return Err(AppError::Refused("Instagram search is off. Turn it on in Settings first.".into()));
            }
            "instagram" => Some(settings.cookie_browser.as_str()),
            _ => None,
        };
        let cache_dir = self.data_dir.join("reference-search");
        let cache = cache_dir.join(format!("{source}-{:016x}.json", ids::fnv1a(query.trim().to_lowercase().as_bytes())));
        if let Ok(modified) = std::fs::metadata(&cache).and_then(|m| m.modified()) {
            if modified.elapsed().unwrap_or(SEARCH_CACHE) < SEARCH_CACHE {
                let cached: Vec<Found> = store::read_or_default(&cache);
                if !cached.is_empty() {
                    return Ok(cached);
                }
            }
        }
        let mut found = ytdlp::search(&self.data_dir, query, source, cookies, SEARCH_LIMIT, &Cancel::default()).await?;
        let thumbs = cache_dir.join("thumbs");
        std::fs::create_dir_all(&thumbs)?;
        let mut fetches = Vec::new();
        for item in &found {
            let target = thumbs.join(format!("{}.jpg", case_safe_id(&item.platform, &item.id)));
            if let Some(url) = item.thumbnail.clone().filter(|url| url.starts_with("https://")) {
                if !target.is_file() {
                    fetches.push(tokio::spawn(fetch(url, target.clone())));
                }
            }
        }
        for fetch in fetches {
            let _ = fetch.await;
        }
        for item in &mut found {
            let target = thumbs.join(format!("{}.jpg", case_safe_id(&item.platform, &item.id)));
            item.thumbnail = target.is_file().then(|| target.to_string_lossy().into_owned());
        }
        store::write_atomic(&cache, &found)?;
        Ok(found)
    }

    /// The assistant's brief for an original video in the reference's format.
    pub fn brief(&self, id: &str, request: &str, narration: Narration) -> AppResult<String> {
        let detail = self.get(id)?;
        let blueprint = detail
            .blueprint
            .ok_or_else(|| AppError::Refused("That reference has not been studied yet.".into()))?;
        let words: Vec<Word> = read_json(&self.dir(id)?.join("transcript.json")).unwrap_or_default();
        Ok(brief_from(&blueprint, &words, request, narration))
    }
}

fn read_json<T: serde::de::DeserializeOwned>(file: &Path) -> AppResult<T> {
    let text = std::fs::read_to_string(file).map_err(|_| AppError::NotFound(format!("{} is missing; study the reference again.", file.file_name().unwrap_or_default().to_string_lossy())))?;
    serde_json::from_str(&text).map_err(|error| AppError::Io(format!("{}: {error}", file.display())))
}

async fn fetch(url: String, target: PathBuf) {
    let Some(curl) = shell_env::find_program("curl") else { return };
    let draft = target.with_extension("part");
    let mut command = shell_env::command(&curl);
    command.args(["-fsSL", "--max-filesize", "2000000", "-o"]).arg(&draft).arg(&url);
    if let Ok(output) = shell_env::run(command, THUMB_LIMIT, "thumbnail").await {
        if output.status.success() {
            let _ = std::fs::rename(&draft, &target);
            return;
        }
    }
    let _ = std::fs::remove_file(&draft);
}

/// Fewer words than this is a few stray sounds, not someone speaking.
const NARRATED_WORDS: usize = 12;

/// What the user chose for narration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Narration<'a> {
    /// Match the reference: a voiceover when it is spoken.
    Auto,
    /// No voiceover.
    Off,
    /// A voiceover in this voice.
    Voice(&'a str),
}

impl<'a> Narration<'a> {
    /// "" means match the reference, "none" means no voice, else a voice id.
    pub fn from_choice(choice: &'a str) -> Self {
        match choice.trim() {
            "" => Self::Auto,
            "none" => Self::Off,
            voice => Self::Voice(voice),
        }
    }
}

/// Whether ours needs a voiceover, and in which voice. The transcript is the
/// evidence of whether the reference is spoken; the blueprint alone does not say.
fn voice_line(words: &[Word], length: f64, narration: Narration) -> String {
    let silent = "carry the message with on-screen text, music and sound.";
    let spoken = words.len() >= NARRATED_WORDS;
    let voice = match narration {
        Narration::Off => return format!("Voice: no voiceover; {silent}"),
        Narration::Auto if !spoken => return format!("Voice: the reference has little or no speech, so {silent}"),
        Narration::Auto => "a voice that suits the subject (`hyperframes tts --list` names them)".to_owned(),
        Narration::Voice(id) => format!("the voice \"{id}\""),
    };
    let evidence = if spoken {
        let first = words.first().map_or(0.0, |word| word.start);
        let last = words.last().map_or(0.0, |word| word.end);
        format!("the reference is spoken — {} words from {first:.1} s to {last:.1} s of its {length:.1} s. ", words.len())
    } else {
        String::new()
    };
    format!(
        "Voice: {evidence}Ours has a voiceover: write an original script for my subject that follows the sections, make it \
with `hyperframes tts` in {voice} (free, runs on this Mac), place it on the timeline and time the scenes to it. Keep the music under the voice."
    )
}

fn brief_from(blueprint: &Blueprint, words: &[Word], request: &str, narration: Narration) -> String {
    let sections = blueprint
        .structure
        .iter()
        .map(|section| format!("- {:.1}–{:.1} s · {}: {}", section.start, section.end, section.role, section.summary))
        .collect::<Vec<_>>()
        .join("\n");
    let shots = blueprint
        .shots
        .iter()
        .map(|shot| {
            let text = shot.analysis.on_screen_text.as_deref().map(|t| format!("; on-screen text: {t}")).unwrap_or_default();
            format!(
                "- {:.1}–{:.1} s ({:.1} s) · {}, {} camera, {} energy: {}{}",
                shot.start, shot.end, shot.duration, shot.analysis.shot_type, shot.analysis.camera_motion, shot.analysis.energy, shot.analysis.subject_action, text
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let pacing = &blueprint.pacing;
    let captions = if blueprint.caption_style.present {
        format!("{} captions, {}{}", blueprint.caption_style.position, blueprint.caption_style.style, blueprint.caption_style.animation.as_deref().map(|a| format!(", animated: {a}")).unwrap_or_default())
    } else {
        "no captions".into()
    };
    let request = request.trim();
    let request = if request.is_empty() { "an original video for me, in this format" } else { request };
    format!(
        "Make {request}.\n\n\
Follow the FORMAT of a reference video I studied. It is a reference only: do not copy its footage, people, \
voice, music, words or on-screen text, and do not try to fetch it. Build everything new for my subject.\n\n\
Format summary: {summary}\n\
Hook (first {hook_window:.0} s, {hook_type}): {hook_visual}. Why it works: {hook_why}\n\
Pacing: {length:.1} s long, {count} shots, average {avg:.1} s a shot ({cuts:.1} cuts every 10 s).\n\
Captions: {captions}.\n\
Music: {mood}, {tempo} tempo (inferred from the visuals; make or choose our own).\n\
{voice}\n\n\
Sections:\n{sections}\n\n\
Shots, in order:\n{shots}\n\n\
Match the pacing and the order of sections closely; adapt the shots to what I have. Plan the scenes first, then build them, then check the result with a snapshot before you finish.",
        summary = blueprint.summary,
        hook_window = blueprint.hook.window_seconds,
        hook_type = blueprint.hook.hook_type,
        hook_visual = blueprint.hook.visual,
        hook_why = blueprint.hook.analysis,
        length = pacing.total_duration,
        count = pacing.shot_count,
        avg = pacing.avg_shot_length,
        cuts = pacing.cuts_per_10s,
        mood = blueprint.music.mood,
        tempo = blueprint.music.tempo,
        voice = voice_line(words, pacing.total_duration, narration),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_that_differ_only_by_case_stay_apart() {
        let (a, b) = (case_safe_id("youtube", "abC"), case_safe_id("youtube", "abc"));
        assert!(a.starts_with("youtube-abc-") && b.starts_with("youtube-abc-"));
        assert_ne!(a.to_lowercase(), b.to_lowercase());
    }

    #[test]
    fn ids_are_safe_folder_names() {
        assert_eq!(slug("YouTube", 12), "youtube");
        assert_eq!(slug("Hello World!! 2", 40), "hello-world-2");
        assert_eq!(slug("--", 10), "ref");
        let service = ReferenceService::new(Path::new("/tmp/rv"));
        assert!(service.dir("youtube-abc_1").is_ok());
        for bad in ["", "../x", "a/b", "a b"] {
            assert!(service.dir(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_failed_step_skips_the_rest_and_a_retry_resumes_there() {
        let mut run = Run::default();
        run.step("download").state = StepState::Done;
        run.step("shots").state = StepState::Failed;
        assert!(!run.finished());
    }

    #[test]
    fn the_brief_forbids_copying_and_carries_the_format() {
        let text = include_str!("testdata/blueprint.json");
        let blueprint: Blueprint = serde_json::from_str(text).unwrap();
        let brief = brief_from(&blueprint, &[], "a launch video for rirote.com", Narration::Auto);
        assert!(brief.starts_with("Make a launch video for rirote.com."));
        assert!(brief.contains("little or no speech"));
        assert!(brief.contains("do not copy its footage"));
        assert!(brief.contains("2 shots, average"));
        assert!(brief.contains("bottom captions, bold white"));
        assert!(brief.contains("- 0.0–3.0 s · hook:"));
    }

    #[test]
    fn a_spoken_reference_asks_for_a_free_voiceover() {
        let blueprint: Blueprint = serde_json::from_str(include_str!("testdata/blueprint.json")).unwrap();
        let words: Vec<Word> =
            (0..40).map(|i| Word { text: format!("w{i}"), start: 1.0 + i as f64 * 0.5, end: 1.4 + i as f64 * 0.5 }).collect();
        let brief = brief_from(&blueprint, &words, "", Narration::Auto);
        assert!(brief.contains("Voice: the reference is spoken — 40 words from 1.0 s to 20.9 s"));
        assert!(brief.contains("`hyperframes tts`"));
        // A few stray words are not narration.
        assert!(brief_from(&blueprint, &words[..3], "", Narration::Auto).contains("little or no speech"));
        // A chosen voice wins either way; "no voice" too.
        assert!(brief_from(&blueprint, &[], "", Narration::Voice("bm_george")).contains("in the voice \"bm_george\""));
        assert!(brief_from(&blueprint, &words, "", Narration::Off).contains("Voice: no voiceover"));
        assert_eq!(Narration::from_choice(""), Narration::Auto);
        assert_eq!(Narration::from_choice("none"), Narration::Off);
    }
}
