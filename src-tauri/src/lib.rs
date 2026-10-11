//! RIROTE VISION — an AI video studio for macOS built on HyperFrames.
//!
//! The window is a web view with no file or process access of its own; every
//! such action is one of the commands below.

mod agent;
mod doctor;
mod error;
mod hyperframes;
mod ids;
mod jobs;
mod log;
mod media;
mod menu;
mod music;
mod preview;
mod project;
mod recents;
mod render;
mod shell_env;
mod skills;
mod timeline;
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
use timeline::{EditOp, TimelineService, TimelineState};

struct AppState {
    /// `~/Library/Application Support/com.riroteglobal.vision`
    data_dir: PathBuf,
    recents: RecentStore,
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

/// Installs HyperFrames' skills for Claude Code, then reports the setup again.
#[tauri::command]
async fn skills_install(state: State<'_, AppState>) -> AppResult<doctor::Report> {
    let _guard = state.install.lock().await;
    if !skills::missing(&skills::folder()).is_empty() {
        skills::install(&state.data_dir).await?;
    }
    Ok(doctor::run(&state.data_dir).await)
}

#[tauri::command]
fn projects_recent(state: State<'_, AppState>) -> Vec<RecentProject> {
    state.recents.list()
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
            app.manage(AppState {
                recents: RecentStore::new(&data_dir),
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
            media_list,
            media_import,
            media_reveal,
            voices_list,
            jobs_list,
            job_start,
            job_cancel,
            job_reveal,
            export_check,
            jobs_clear_finished,
            launch_project,
            launch_autoplay,
            client_log,
        ])
        .build(tauri::generate_context!())
        .expect("RIROTE VISION could not start")
        .run(|app, event| {
            // Neither the preview server nor a running turn may outlive the app.
            if let RunEvent::Exit = event {
                let state = app.state::<AppState>();
                state.agent.stop();
                state.jobs.stop_all();
                tauri::async_runtime::block_on(state.preview.stop());
            }
        });
}
