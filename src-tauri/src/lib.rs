//! RIROTE VISION — an AI video studio for macOS built on HyperFrames.
//!
//! The window is a web view with no file or process access of its own; every
//! such action is one of the commands below.

mod agent;
mod brandkit;
mod catalog;
mod comments;
mod doctor;
mod error;
mod history;
mod hyperframes;
mod ids;
mod jobs;
mod log;
mod looks;
mod media;
mod media_cache;
mod menu;
mod music;
mod openverse;
mod posters;
mod preview;
mod project;
mod recents;
mod reference;
mod render;
mod settings;
mod shell_env;
mod skills;
mod store;
mod timeline;
mod transcript;
mod voice;

#[cfg(test)]
mod live_tests;

use std::path::{Path, PathBuf};

use std::sync::Arc;

use serde::Deserialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};

use agent::{AgentEvent, AgentService, Answer, ChatView, ClaudeCode, EditorContext, Trust};
use error::AppResult;
use jobs::{Job, JobQueue, JobSpec};
use media::{MediaItem, MediaLibrary};
use preview::{PreviewEvent, PreviewManager, PreviewSession};
use project::{ProjectInfo, ProjectSummary, Resolution};
use recents::{RecentProject, RecentStore};
use settings::{Settings, SettingsStore};
use timeline::{EditOp, TimelineService, TimelineState};

struct AppState {
    /// `~/Library/Application Support/com.riroteglobal.vision`
    data_dir: PathBuf,
    recents: RecentStore,
    settings: SettingsStore,
    posters: posters::Posters,
    brands: brandkit::BrandKits,
    comments: comments::Comments,
    references: Arc<reference::ReferenceService>,
    /// One engine install at a time.
    install: tokio::sync::Mutex<()>,
    preview: PreviewManager,
    timeline: TimelineService,
    agent: AgentService,
    media: MediaLibrary,
    jobs: Arc<JobQueue>,
    /// A project folder given on the command line, opened once at launch.
    launch_project: std::sync::Mutex<Option<String>>,
}

#[tauri::command]
async fn environment_check(state: State<'_, AppState>) -> AppResult<doctor::Report> {
    Ok(doctor::run(&state.data_dir).await)
}

/// Installs the pinned video engine, then reports the setup again.
#[tauri::command]
async fn engine_install(state: State<'_, AppState>) -> AppResult<doctor::Report> {
    let _guard = state.install.lock().await;
    if !hyperframes::is_ready(&state.data_dir) {
        hyperframes::install(&state.data_dir).await?;
    }
    Ok(doctor::run(&state.data_dir).await)
}

/// Installs the voiceover runtime, then reports the setup again.
#[tauri::command]
async fn voice_install(state: State<'_, AppState>) -> AppResult<doctor::Report> {
    let _guard = state.install.lock().await;
    if voice::python(&state.data_dir).is_none() {
        voice::install(&state.data_dir).await?;
    }
    Ok(doctor::run(&state.data_dir).await)
}

/// A short spoken sample of a voice, made once; returns the file to play.
#[tauri::command]
async fn voice_sample(state: State<'_, AppState>, voice: String) -> AppResult<String> {
    Ok(voice::sample(&state.data_dir, &voice).await?.to_string_lossy().into_owned())
}

/// Installs HyperFrames' skills for Claude Code, then reports the setup again.
#[tauri::command]
async fn skills_install(state: State<'_, AppState>) -> AppResult<doctor::Report> {
    let _guard = state.install.lock().await;
    if !skills::missing(&skills::folder()).is_empty() {
        skills::install(&state.data_dir).await?;
    }
    Ok(doctor::run(&state.data_dir).await)
}

/// Installs or updates yt-dlp, then reports the setup again.
#[tauri::command]
async fn ytdlp_install(state: State<'_, AppState>) -> AppResult<doctor::Report> {
    let _guard = state.install.lock().await;
    reference::ytdlp::install(&state.data_dir).await?;
    Ok(doctor::run(&state.data_dir).await)
}

#[tauri::command]
fn reference_list(state: State<'_, AppState>) -> Vec<reference::model::Reference> {
    state.references.list()
}

#[tauri::command]
fn reference_get(state: State<'_, AppState>, id: String) -> AppResult<reference::ReferenceDetail> {
    state.references.get(&id)
}

#[tauri::command]
async fn reference_import(
    state: State<'_, AppState>,
    link: Option<String>,
    file: Option<String>,
) -> AppResult<reference::model::Reference> {
    let settings = state.settings.get();
    state.references.import(link, file, &settings).await
}

#[tauri::command]
fn reference_retry(state: State<'_, AppState>, id: String) -> AppResult<reference::model::Reference> {
    state.references.retry(&id)
}

#[tauri::command]
fn reference_stop(state: State<'_, AppState>, id: String) {
    state.references.stop(&id);
}

#[tauri::command]
fn reference_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.references.delete(&id)
}

#[tauri::command]
fn reference_brief(state: State<'_, AppState>, id: String, request: String, voice: Option<String>) -> AppResult<String> {
    let voice = voice.unwrap_or_default();
    state.references.brief(&id, &request, reference::Narration::from_choice(&voice))
}

#[tauri::command]
fn projects_recent(state: State<'_, AppState>) -> Vec<RecentProject> {
    state.recents.list()
}

#[tauri::command]
async fn project_poster(state: State<'_, AppState>, path: String) -> AppResult<String> {
    state.posters.get(&state.data_dir, Path::new(&path)).await
}

#[tauri::command]
fn projects_folder(state: State<'_, AppState>) -> AppResult<String> {
    Ok(state.settings.projects_folder()?.to_string_lossy().into_owned())
}

#[tauri::command]
async fn media_peaks(state: State<'_, AppState>, path: String, src: String) -> AppResult<media_cache::Peaks> {
    media_cache::peaks(&state.data_dir, Path::new(&path), &src).await
}

#[tauri::command]
async fn media_stills(state: State<'_, AppState>, path: String, src: String) -> AppResult<Vec<media_cache::Still>> {
    media_cache::stills(&state.data_dir, Path::new(&path), &src).await
}

#[tauri::command]
async fn catalog_list(state: State<'_, AppState>, shelf: String) -> AppResult<Vec<catalog::Item>> {
    catalog::list(&state.data_dir, &shelf).await
}

#[tauri::command]
async fn catalog_add(state: State<'_, AppState>, path: String, name: String) -> AppResult<catalog::Installed> {
    catalog::add(&state.data_dir, Path::new(&path), &name).await
}

#[tauri::command]
fn transcript_read(path: String, src: String) -> AppResult<Option<transcript::Transcript>> {
    transcript::read(Path::new(&path), &src)
}

#[tauri::command]
async fn media_silences(path: String, src: String, noise_db: f64, min_seconds: f64) -> AppResult<Vec<(f64, f64)>> {
    transcript::silences(Path::new(&path), &src, noise_db, min_seconds).await
}

#[tauri::command]
async fn timeline_cut(
    state: State<'_, AppState>,
    path: String,
    clip: String,
    ranges: Vec<(f64, f64)>,
) -> AppResult<TimelineState> {
    state.timeline.cut(&state.data_dir, Path::new(&path), &clip, &ranges).await
}

#[tauri::command]
async fn looks_list(state: State<'_, AppState>) -> AppResult<Vec<looks::Look>> {
    looks::list(&state.data_dir).await
}

#[tauri::command]
async fn look_apply(
    state: State<'_, AppState>,
    path: String,
    clip_id: String,
    preset: Option<String>,
    intensity: f64,
) -> AppResult<()> {
    looks::apply(&state.data_dir, Path::new(&path), &clip_id, preset.as_deref(), intensity).await
}

#[tauri::command]
fn brandkit_list(state: State<'_, AppState>) -> Vec<brandkit::Kit> {
    state.brands.list()
}

#[tauri::command]
fn brandkit_save(state: State<'_, AppState>, kit: brandkit::Kit) -> AppResult<brandkit::Kit> {
    state.brands.save(kit)
}

#[tauri::command]
fn brandkit_logo(state: State<'_, AppState>, id: String, file: String) -> AppResult<brandkit::Kit> {
    state.brands.set_logo(&id, Path::new(&file))
}

#[tauri::command]
fn brandkit_add_asset(state: State<'_, AppState>, id: String, file: String, label: String, note: String) -> AppResult<brandkit::Kit> {
    state.brands.add_asset(&id, Path::new(&file), &label, &note)
}

#[tauri::command]
fn brandkit_update_asset(state: State<'_, AppState>, id: String, asset: String, label: String, note: String) -> AppResult<brandkit::Kit> {
    state.brands.update_asset(&id, &asset, &label, &note)
}

#[tauri::command]
fn brandkit_remove_asset(state: State<'_, AppState>, id: String, asset: String) -> AppResult<brandkit::Kit> {
    state.brands.remove_asset(&id, &asset)
}

/// Makes a background-removed copy of a kit photo. The first run downloads
/// the engine's cut-out model, so it may take a few minutes.
#[tauri::command]
async fn brandkit_cutout(state: State<'_, AppState>, id: String, asset: String) -> AppResult<brandkit::Kit> {
    let (source, output) = state.brands.cutout_plan(&id, &asset)?;
    let folder = output.parent().map(Path::to_path_buf).unwrap_or_else(|| state.data_dir.clone());
    let args = ["remove-background", &source.to_string_lossy(), "-o", &output.to_string_lossy(), "--json"].map(|a| a.to_string());
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let ran = hyperframes::run(&state.data_dir, &args, &folder, std::time::Duration::from_secs(15 * 60)).await;
    let made = matches!(&ran, Ok(out) if out.status.success()) && output.is_file();
    if !made {
        let _ = std::fs::remove_file(&output);
        let why = match ran {
            Ok(out) => shell_env::failure_text(&out),
            Err(error) => error.to_string(),
        };
        log::write(&state.data_dir, "brandkit", &format!("cut-out failed: {why}"));
        return Err(error::AppError::ToolFailed(format!("The background could not be removed. {}", shell_env::first_line(&why))));
    }
    state.brands.add_cutout(&id, &asset, &output)
}

#[tauri::command]
fn brandkit_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.brands.delete(&id)
}

#[tauri::command]
fn project_brand_get(state: State<'_, AppState>, path: String) -> Option<brandkit::Kit> {
    state.brands.of_project(Path::new(&path))
}

#[tauri::command]
fn project_brand_set(state: State<'_, AppState>, path: String, id: Option<String>) -> AppResult<Option<brandkit::Kit>> {
    state.brands.use_in(Path::new(&path), id.as_deref())
}

#[tauri::command]
fn brandkit_brief(state: State<'_, AppState>, id: String) -> AppResult<String> {
    Ok(state.brands.get(&id)?.brief())
}

#[tauri::command]
async fn history_list(state: State<'_, AppState>, path: String) -> AppResult<Vec<history::Entry>> {
    history::list(&state.data_dir, Path::new(&path)).await
}

#[tauri::command]
async fn history_undo(state: State<'_, AppState>, path: String, id: String) -> AppResult<()> {
    history::undo(&state.data_dir, Path::new(&path), &id).await
}

#[tauri::command]
fn comments_list(state: State<'_, AppState>, path: String) -> Vec<comments::Comment> {
    state.comments.list(Path::new(&path))
}

#[tauri::command]
fn comments_add(
    state: State<'_, AppState>,
    path: String,
    time: f64,
    x: Option<f64>,
    y: Option<f64>,
    text: String,
) -> AppResult<Vec<comments::Comment>> {
    state.comments.add(Path::new(&path), time, x, y, &text)
}

#[tauri::command]
fn comments_update(
    state: State<'_, AppState>,
    path: String,
    id: String,
    text: Option<String>,
    resolved: Option<bool>,
) -> AppResult<Vec<comments::Comment>> {
    state.comments.update(Path::new(&path), &id, text.as_deref(), resolved)
}

#[tauri::command]
fn comments_delete(state: State<'_, AppState>, path: String, id: String) -> AppResult<Vec<comments::Comment>> {
    state.comments.delete(Path::new(&path), &id)
}

#[tauri::command]
fn skills_list() -> Vec<skills::SkillInfo> {
    skills::list(&skills::folder())
}

#[tauri::command]
async fn sounds_search(query: String, commercial: bool) -> AppResult<Vec<openverse::Sound>> {
    openverse::search(&query, commercial).await
}

#[tauri::command]
async fn sounds_download(path: String, sound: openverse::Sound) -> AppResult<String> {
    openverse::download(Path::new(&path), &sound).await
}

#[tauri::command]
fn settings_get(state: State<'_, AppState>) -> Settings {
    state.settings.get()
}

#[tauri::command]
fn settings_set(state: State<'_, AppState>, settings: Settings) -> AppResult<Settings> {
    state.settings.set(settings)
}

#[tauri::command]
fn project_open(state: State<'_, AppState>, path: String) -> AppResult<ProjectSummary> {
    let summary = project::inspect(Path::new(&path))?;
    state.recents.touch(&summary)?;
    Ok(summary)
}

#[derive(Deserialize)]
struct NewProject {
    parent: String,
    name: String,
    resolution: Resolution,
}

#[tauri::command]
async fn project_create(state: State<'_, AppState>, input: NewProject) -> AppResult<ProjectSummary> {
    let summary =
        project::create(&state.data_dir, Path::new(&input.parent), &input.name, input.resolution)
            .await?;
    state.recents.touch(&summary)?;
    Ok(summary)
}

#[tauri::command]
fn project_forget(state: State<'_, AppState>, path: String) -> AppResult<()> {
    state.recents.forget(&path)
}

#[tauri::command]
async fn project_info(state: State<'_, AppState>, path: String) -> AppResult<ProjectInfo> {
    project::info(&state.data_dir, Path::new(&path)).await
}

#[tauri::command]
async fn project_reveal(path: String) -> AppResult<()> {
    project::reveal(Path::new(&path)).await
}

/// Takes a hold on a project's preview server and reports its changes to the
/// window as `preview:changed` and `preview:stopped` events.
#[tauri::command]
async fn preview_start(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    fresh: bool,
) -> AppResult<PreviewSession> {
    let summary = project::inspect(Path::new(&path))?;
    let listener = Arc::new(move |event: PreviewEvent| {
        let _ = match event {
            PreviewEvent::Changed { project_path, signature } => app.emit(
                "preview:changed",
                json!({ "projectPath": project_path, "signature": signature }),
            ),
            PreviewEvent::Stopped { project_path } => {
                app.emit("preview:stopped", json!({ "projectPath": project_path }))
            }
        };
    });
    state.preview.start(Path::new(&summary.path), fresh, listener).await
}

/// Gives back the hold a `preview_start` returned.
#[tauri::command]
async fn preview_release(state: State<'_, AppState>, lease: u64) -> AppResult<()> {
    state.preview.release(lease).await;
    Ok(())
}

#[tauri::command]
async fn timeline_read(state: State<'_, AppState>, path: String) -> AppResult<TimelineState> {
    let summary = project::inspect(Path::new(&path))?;
    state.timeline.read(&state.data_dir, Path::new(&summary.path)).await
}

#[tauri::command]
async fn timeline_edit(
    state: State<'_, AppState>,
    path: String,
    op: EditOp,
) -> AppResult<TimelineState> {
    let summary = project::inspect(Path::new(&path))?;
    state.timeline.edit(&state.data_dir, Path::new(&summary.path), op).await
}

#[tauri::command]
async fn timeline_undo(state: State<'_, AppState>, path: String) -> AppResult<TimelineState> {
    let summary = project::inspect(Path::new(&path))?;
    state.timeline.undo(&state.data_dir, Path::new(&summary.path)).await
}

#[tauri::command]
async fn timeline_redo(state: State<'_, AppState>, path: String) -> AppResult<TimelineState> {
    let summary = project::inspect(Path::new(&path))?;
    state.timeline.redo(&state.data_dir, Path::new(&summary.path)).await
}

#[tauri::command]
fn chat_load(state: State<'_, AppState>, path: String) -> AppResult<ChatView> {
    let summary = project::inspect(Path::new(&path))?;
    Ok(state.agent.view(Path::new(&summary.path)))
}

/// Sends a request to the assistant. Its progress reaches the window as
/// `agent` events carrying the project's path.
#[tauri::command]
async fn chat_send(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    prompt: String,
    context: EditorContext,
) -> AppResult<()> {
    let summary = project::inspect(Path::new(&path))?;
    let listener = Arc::new(move |project: &Path, event: AgentEvent| {
        let _ = app.emit("agent", json!({ "projectPath": project, "event": event }));
    });
    state.agent.send(Path::new(&summary.path), &prompt, &context, listener).await
}

#[tauri::command]
async fn chat_answer(state: State<'_, AppState>, request_id: String, answer: Answer) -> AppResult<()> {
    state.agent.answer(&request_id, answer).await
}

#[tauri::command]
fn chat_stop(state: State<'_, AppState>) {
    state.agent.stop();
}

/// Sets how much the assistant may do in a project without asking.
#[tauri::command]
fn chat_set_trust(state: State<'_, AppState>, path: String, trust: Trust) -> AppResult<ChatView> {
    let summary = project::inspect(Path::new(&path))?;
    state.agent.set_trust(Path::new(&summary.path), trust)
}

#[tauri::command]
fn chat_clear(state: State<'_, AppState>, path: String) -> AppResult<ChatView> {
    let summary = project::inspect(Path::new(&path))?;
    state.agent.clear(Path::new(&summary.path))
}

#[tauri::command]
fn chat_forget_rules(state: State<'_, AppState>, path: String) -> AppResult<ChatView> {
    let summary = project::inspect(Path::new(&path))?;
    state.agent.forget_rules(Path::new(&summary.path))
}

#[tauri::command]
async fn media_list(state: State<'_, AppState>, path: String) -> AppResult<Vec<MediaItem>> {
    let summary = project::inspect(Path::new(&path))?;
    state.media.list(Path::new(&summary.path)).await
}

/// Copies files chosen or dropped by the user into the project.
#[tauri::command]
async fn media_import(
    state: State<'_, AppState>,
    path: String,
    sources: Vec<String>,
) -> AppResult<Vec<MediaItem>> {
    let summary = project::inspect(Path::new(&path))?;
    let sources: Vec<PathBuf> = sources.iter().map(PathBuf::from).collect();
    state.media.import(Path::new(&summary.path), &sources).await
}

#[tauri::command]
async fn media_reveal(path: String, file: String) -> AppResult<()> {
    let summary = project::inspect(Path::new(&path))?;
    let target = media::resolve(Path::new(&summary.path), &file)?;
    project::reveal_file(&target).await
}

/// The composition files that use a media file, so the window can warn before deleting it.
#[tauri::command]
fn media_used_by(path: String, file: String) -> AppResult<Vec<String>> {
    let summary = project::inspect(Path::new(&path))?;
    media::used_by(Path::new(&summary.path), &file)
}

/// Moves a media file to the Trash.
#[tauri::command]
fn media_trash(path: String, file: String) -> AppResult<()> {
    let summary = project::inspect(Path::new(&path))?;
    media::trash(Path::new(&summary.path), &file)
}

/// The voices the engine's text-to-speech offers.
#[tauri::command]
async fn voices_list(state: State<'_, AppState>) -> AppResult<Vec<serde_json::Value>> {
    let output = hyperframes::run(
        &state.data_dir,
        &["tts", "--list", "--json"],
        &state.data_dir,
        std::time::Duration::from_secs(30),
    )
    .await?;
    let text = String::from_utf8_lossy(&output.stdout);
    let voices = text.find('[').and_then(|start| serde_json::from_str(&text[start..]).ok());
    voices.ok_or_else(|| error::AppError::ToolFailed("The engine did not list its voices.".into()))
}

#[tauri::command]
fn jobs_list(state: State<'_, AppState>, path: String) -> AppResult<Vec<Job>> {
    let summary = project::inspect(Path::new(&path))?;
    Ok(state.jobs.list(Path::new(&summary.path)))
}

/// Queues a media job. Its progress reaches the window as `jobs` events.
#[tauri::command]
fn job_start(state: State<'_, AppState>, path: String, spec: JobSpec) -> AppResult<Job> {
    let summary = project::inspect(Path::new(&path))?;
    state.jobs.add(Path::new(&summary.path), spec)
}

#[tauri::command]
fn job_cancel(state: State<'_, AppState>, id: u64) {
    state.jobs.cancel(id);
}

/// Selects the file a finished job made in Finder.
#[tauri::command]
async fn job_reveal(state: State<'_, AppState>, path: String, id: u64) -> AppResult<()> {
    let summary = project::inspect(Path::new(&path))?;
    let jobs = state.jobs.list(Path::new(&summary.path));
    let file = jobs
        .iter()
        .find(|job| job.id == id)
        .and_then(|job| job.file.clone())
        .map(PathBuf::from)
        .filter(|file| file.is_file())
        .ok_or_else(|| error::AppError::NotFound("That file is no longer there.".into()))?;
    project::reveal_file(&file).await
}

/// Runs the engine's checks on the composition, ahead of an export.
#[tauri::command]
async fn export_check(state: State<'_, AppState>, path: String) -> AppResult<render::CheckSummary> {
    let summary = project::inspect(Path::new(&path))?;
    render::check(&state.data_dir, Path::new(&summary.path)).await
}

#[tauri::command]
fn jobs_clear_finished(state: State<'_, AppState>, path: String) -> AppResult<()> {
    let summary = project::inspect(Path::new(&path))?;
    state.jobs.clear_finished(Path::new(&summary.path));
    Ok(())
}

/// The project folder the app was launched with, if any. Answers once.
#[tauri::command]
fn launch_project(state: State<'_, AppState>) -> Option<String> {
    state.launch_project.lock().ok()?.take()
}

/// True when the app was started with `--autoplay`: the preview then plays
/// by itself once it has loaded, so playback can be checked from the log
/// without anyone at the window.
#[tauri::command]
fn launch_autoplay() -> bool {
    std::env::args().any(|argument| argument == "--autoplay")
}

/// Lets the window record what it saw, next to the app's own log lines.
#[tauri::command]
fn client_log(state: State<'_, AppState>, message: String) {
    let message: String = message.chars().take(2000).collect();
    log::write(&state.data_dir, "window", &message);
}

/// The first command-line argument that is a folder.
fn project_argument() -> Option<String> {
    std::env::args().skip(1).find(|argument| Path::new(argument).is_dir())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let jobs = Arc::new(JobQueue::new(&data_dir));
            let window = app.handle().clone();
            jobs.listen(Arc::new(move |list: Vec<Job>| {
                let _ = window.emit("jobs", list);
            }));
            tauri::async_runtime::spawn(jobs.clone().work());
            let references = Arc::new(reference::ReferenceService::new(&data_dir));
            let window = app.handle().clone();
            references.listen(Arc::new(move |list| {
                let _ = window.emit("references", list);
            }));
            let settings_dir = data_dir.clone();
            tauri::async_runtime::spawn(references.clone().work(Arc::new(move || SettingsStore::new(&settings_dir).get())));
            app.manage(AppState {
                recents: RecentStore::new(&data_dir),
                settings: SettingsStore::new(&data_dir),
                posters: posters::Posters::new(&data_dir),
                brands: brandkit::BrandKits::new(&data_dir),
                comments: comments::Comments::new(),
                references,
                preview: PreviewManager::new(&data_dir),
                timeline: TimelineService::new(&data_dir),
                agent: AgentService::new(&data_dir, Arc::new(ClaudeCode::default())),
                media: MediaLibrary::new(),
                jobs,
                data_dir,
                install: tokio::sync::Mutex::new(()),
                launch_project: std::sync::Mutex::new(project_argument()),
            });
            menu::install(app.handle())?;
            // Reading the login shell's PATH takes a moment; do it before the
            // first command needs it.
            std::thread::spawn(|| {
                shell_env::search_path();
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            environment_check,
            engine_install,
            projects_recent,
            project_open,
            project_create,
            project_forget,
            project_info,
            project_reveal,
            preview_start,
            preview_release,
            timeline_read,
            timeline_edit,
            timeline_undo,
            timeline_redo,
            chat_load,
            chat_send,
            chat_answer,
            chat_stop,
            chat_set_trust,
            chat_clear,
            chat_forget_rules,
            voice_install,
            skills_install,
            ytdlp_install,
            reference_list,
            reference_get,
            reference_import,
            reference_retry,
            reference_stop,
            reference_delete,
            reference_brief,
            media_list,
            media_import,
            media_reveal,
            media_used_by,
            media_trash,
            voices_list,
            voice_sample,
            jobs_list,
            job_start,
            job_cancel,
            job_reveal,
            export_check,
            jobs_clear_finished,
            launch_project,
            launch_autoplay,
            client_log,
            settings_get,
            sounds_search,
            sounds_download,
            comments_list,
            comments_add,
            comments_update,
            comments_delete,
            skills_list,
            history_list,
            history_undo,
            looks_list,
            look_apply,
            brandkit_list,
            brandkit_save,
            brandkit_logo,
            brandkit_add_asset,
            brandkit_update_asset,
            brandkit_remove_asset,
            brandkit_cutout,
            brandkit_delete,
            brandkit_brief,
            project_brand_get,
            project_brand_set,
            transcript_read,
            media_silences,
            timeline_cut,
            media_peaks,
            media_stills,
            catalog_list,
            catalog_add,
            project_poster,
            projects_folder,
            settings_set,
        ])
        .build(tauri::generate_context!())
        .expect("RIROTE VISION could not start")
        .run(|app, event| {
            // Neither the preview server nor a running turn may outlive the app.
            if let RunEvent::Exit = event {
                let state = app.state::<AppState>();
                state.agent.stop();
                state.jobs.stop_all();
                state.references.stop_all();
                tauri::async_runtime::block_on(state.preview.stop());
            }
        });
}
