//! The preview: one HyperFrames studio server for the open project, started
//! on a free local port and stopped when the project closes or the app quits.
//!
//! The server compiles the composition and serves it at a URL the window
//! plays in a frame. A watcher looks at the project's files once a second
//! and reports when one that can change the picture or the sound has
//! changed, which is how edits made by anything — the timeline, the
//! assistant, another editor — reach the preview.
//!
//! The server has a change signature of its own, but it also moves when
//! Finder writes a `.DS_Store`, when a snapshot is saved or when a text file
//! is edited, and each of those would restart the preview for nothing.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Child;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::error::{AppError, AppResult};
use crate::{hyperframes, ids, log};

const START_TIMEOUT: Duration = Duration::from_secs(60);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(4);
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const WATCH_INTERVAL: Duration = Duration::from_secs(1);
/// Unanswered requests in a row before the server counts as gone.
const WATCH_FAILURES: u32 = 5;
/// Folders whose contents never reach the picture.
const UNWATCHED_DIRS: [&str; 5] = ["node_modules", "renders", "snapshots", "out", "dist"];
/// The kinds of file a composition is made of.
const WATCHED_EXTENSIONS: [&str; 29] = [
    "html", "css", "js", "mjs", "json", "svg", "png", "jpg", "jpeg", "webp", "gif", "avif", "mp4", "mov",
    "webm", "mkv", "m4v", "wav", "mp3", "m4a", "aac", "ogg", "flac", "woff", "woff2", "ttf", "otf", "srt", "vtt",
];
const MAX_WATCH_DEPTH: usize = 6;
const MAX_WATCHED_FILES: usize = 5_000;
const STOP_GRACE: Duration = Duration::from_secs(3);
const HOST: &str = "127.0.0.1";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSession {
    pub project_path: String,
    /// The compiled composition, playable in a frame.
    pub url: String,
    /// Changes whenever the project's files change.
    pub signature: String,
    /// This caller's hold on the server; hand it back with `release`.
    pub lease: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PreviewEvent {
    /// The project changed on disk; the picture is out of date.
    Changed { project_path: String, signature: String },
    /// The server stopped answering.
    Stopped { project_path: String },
}

pub type Listener = Arc<dyn Fn(PreviewEvent) + Send + Sync>;

struct Running {
    child: Child,
    session: PreviewSession,
    watcher: JoinHandle<()>,
    /// Holds not yet released. The server stops when the last one is.
    leases: HashSet<u64>,
}

pub struct PreviewManager {
    data_dir: PathBuf,
    current: Mutex<Option<Running>>,
    next_lease: AtomicU64,
}

impl PreviewManager {
    pub fn new(data_dir: &Path) -> Self {
        Self { data_dir: data_dir.to_path_buf(), current: Mutex::new(None), next_lease: AtomicU64::new(1) }
    }

    /// Takes a hold on the preview for `project_dir`, starting the server if
    /// it is not running. `fresh` replaces a running server with a new one.
    /// Any other project's preview is stopped first.
    ///
    /// Holds exist because the window can ask to stop one view of a project
    /// and start another at the same moment, in either order: a late stop
    /// must not take down the server a newer view is using.
    pub async fn start(
        &self,
        project_dir: &Path,
        fresh: bool,
        listener: Listener,
    ) -> AppResult<PreviewSession> {
        let mut current = self.current.lock().await;
        let project_path = project_dir.to_string_lossy().into_owned();
        let lease = self.next_lease.fetch_add(1, Ordering::Relaxed);

        if let Some(running) = current.as_mut() {
            let alive = matches!(running.child.try_wait(), Ok(None));
            if alive && !fresh && running.session.project_path == project_path {
                running.leases.insert(lease);
                return Ok(PreviewSession { lease, ..running.session.clone() });
            }
        }
        if let Some(running) = current.take() {
            stop(running).await;
        }

        // Best effort: without it the preview still runs, only less smoothly.
        // RIROTE_VISION_NO_TUNE leaves the engine as installed, for comparing.
        let tuned = if std::env::var_os("RIROTE_VISION_NO_TUNE").is_some() {
            Ok(0)
        } else {
            hyperframes::tune_preview(&self.data_dir)
        };
        match tuned {
            Ok(0) => {}
            Ok(files) => log::write(&self.data_dir, "preview", &format!("sync tolerance applied to {files} engine file(s)")),
            Err(error) => log::write(&self.data_dir, "preview", &format!("sync tolerance not applied: {error}")),
        }
        let port = free_port().await?;
        let mut child = self.spawn(project_dir, port)?;
        let session = match self.wait_until_ready(&mut child, project_dir, port, lease).await {
            Ok(session) => session,
            Err(error) => {
                stop_child(&mut child).await;
                log::write(&self.data_dir, "preview", &format!("start failed: {error}"));
                return Err(error);
            }
        };
        log::write(&self.data_dir, "preview", &format!("serving {project_path} on port {port}"));

        let watcher = tokio::spawn(watch(port, project_dir.to_path_buf(), session.clone(), listener));
        *current =
            Some(Running { child, session: session.clone(), watcher, leases: HashSet::from([lease]) });
        Ok(session)
    }

    /// Gives a hold back. The server stops when nobody holds it.
    pub async fn release(&self, lease: u64) {
        let mut current = self.current.lock().await;
        let Some(running) = current.as_mut() else { return };
        if running.leases.remove(&lease) && running.leases.is_empty() {
            if let Some(running) = current.take() {
                log::write(&self.data_dir, "preview", &format!("stopped {}", running.session.project_path));
                stop(running).await;
            }
        }
    }

    /// Stops the server whoever holds it. For when the app quits.
    pub async fn stop(&self) {
        if let Some(running) = self.current.lock().await.take() {
            log::write(&self.data_dir, "preview", &format!("stopped {}", running.session.project_path));
            stop(running).await;
        }
    }

    fn log_file(&self) -> PathBuf {
        log::dir(&self.data_dir).join("preview-server.log")
    }

    /// The server gets its own process group so that stopping it also stops
    /// the helpers it starts. Its output goes to a file: an unread pipe
    /// would fill up and stall it.
    fn spawn(&self, project_dir: &Path, port: u16) -> AppResult<Child> {
        std::fs::create_dir_all(log::dir(&self.data_dir))?;
        let output = std::fs::File::create(self.log_file())?;
        let port = port.to_string();
        let args = ["preview", "--no-open", "--foreground", "--force-new", "--port", port.as_str()];
        let mut command = hyperframes::command(&self.data_dir, &args, project_dir)?;
        command
            .env("HYPERFRAMES_PREVIEW_HOST", HOST)
            .stdout(Stdio::from(output.try_clone()?))
            .stderr(Stdio::from(output))
            .process_group(0);
        command
            .spawn()
            .map_err(|error| AppError::ToolFailed(format!("The preview could not start: {error}")))
    }

    async fn wait_until_ready(
        &self,
        child: &mut Child,
        project_dir: &Path,
        port: u16,
        lease: u64,
    ) -> AppResult<PreviewSession> {
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                return Err(AppError::ToolFailed(format!(
                    "The preview stopped while starting ({status}).\n{}",
                    tail(&self.log_file())
                )));
            }
            if let Ok(body) = http_get(port, "/api/projects").await {
                if let Some(id) = project_id(&body, project_dir) {
                    let base = format!("/api/projects/{}", encode_segment(&id));
                    let signature = project_signature(project_dir);
                    return Ok(PreviewSession {
                        project_path: project_dir.to_string_lossy().into_owned(),
                        url: format!("http://{HOST}:{port}{base}/preview"),
                        signature,
                        lease,
                    });
                }
            }
            if started.elapsed() > START_TIMEOUT {
                return Err(AppError::Timeout(format!(
                    "The preview did not start within {} seconds.\n{}",
                    START_TIMEOUT.as_secs(),
                    tail(&self.log_file())
                )));
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }
}

async fn stop(mut running: Running) {
    running.watcher.abort();
    stop_child(&mut running.child).await;
}

/// Asks the server's whole process group to end, then insists.
async fn stop_child(child: &mut Child) {
    let Some(pid) = child.id().and_then(|pid| i32::try_from(pid).ok()) else {
        return; // already ended
    };
    // SAFETY: `pid` is this child's own process group, created by
    // `process_group(0)`; signalling it cannot reach unrelated processes.
    unsafe { libc::killpg(pid, libc::SIGTERM) };
    if tokio::time::timeout(STOP_GRACE, child.wait()).await.is_err() {
        // SAFETY: as above.
        unsafe { libc::killpg(pid, libc::SIGKILL) };
        let _ = child.wait().await;
    }
}

/// Reports changes to the project's files until the server stops answering.
async fn watch(port: u16, project_dir: PathBuf, session: PreviewSession, listener: Listener) {
    let mut last = session.signature;
    let mut failures = 0;
    loop {
        tokio::time::sleep(WATCH_INTERVAL).await;
        if http_get(port, "/api/projects").await.is_err() {
            failures += 1;
            if failures >= WATCH_FAILURES {
                listener(PreviewEvent::Stopped { project_path: session.project_path.clone() });
                return;
            }
            continue;
        }
        failures = 0;
        let dir = project_dir.clone();
        let Ok(signature) = tokio::task::spawn_blocking(move || project_signature(&dir)).await else {
            continue;
        };
        if signature != last {
            last = signature.clone();
            listener(PreviewEvent::Changed { project_path: session.project_path.clone(), signature });
        }
    }
}

/// A value that changes when a file the composition is made of is added,
/// removed, resized or rewritten.
pub fn project_signature(project_dir: &Path) -> String {
    let mut seen = Vec::new();
    collect_watched(project_dir, project_dir, 0, &mut seen);
    seen.sort();
    format!("{:016x}", ids::fnv1a(seen.join("\n").as_bytes()))
}

fn collect_watched(root: &Path, dir: &Path, depth: usize, seen: &mut Vec<String>) {
    if depth > MAX_WATCH_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.filter_map(Result::ok) {
        if seen.len() >= MAX_WATCHED_FILES {
            return;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            if !UNWATCHED_DIRS.contains(&name.as_str()) {
                collect_watched(root, &path, depth + 1, seen);
            }
            continue;
        }
        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("").to_ascii_lowercase();
        // package-lock.json and friends churn without touching the film.
        if !WATCHED_EXTENSIONS.contains(&extension.as_str()) || name.ends_with("-lock.json") {
            continue;
        }
        let modified = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|since| since.as_nanos())
            .unwrap_or(0);
        let inside = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().into_owned();
        seen.push(format!("{inside}|{}|{modified}", meta.len()));
    }
}

/// A port nobody is using right now.
async fn free_port() -> AppResult<u16> {
    let listener = tokio::net::TcpListener::bind((HOST, 0)).await?;
    Ok(listener.local_addr()?.port())
}

/// A small GET against the local server. HTTP/1.0 keeps the answer simple:
/// the server sends the body whole and closes the connection.
async fn http_get(port: u16, path: &str) -> AppResult<String> {
    let request = async {
        let mut stream = TcpStream::connect((HOST, port)).await?;
        stream
            .write_all(format!("GET {path} HTTP/1.0\r\nHost: {HOST}\r\nAccept: */*\r\n\r\n").as_bytes())
            .await?;
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await?;
        Ok::<_, std::io::Error>(response)
    };
    let response = tokio::time::timeout(REQUEST_TIMEOUT, request)
        .await
        .map_err(|_| AppError::Timeout("The preview server did not answer.".into()))??;
    parse_response(&String::from_utf8_lossy(&response))
        .ok_or_else(|| AppError::ToolFailed("The preview server gave an unexpected answer.".into()))
}

/// The body of a 200 response.
fn parse_response(response: &str) -> Option<String> {
    let (head, body) = response.split_once("\r\n\r\n")?;
    let status = head.lines().next()?.split_whitespace().nth(1)?;
    (status == "200").then(|| body.to_owned())
}

/// The server's id for `project_dir`, from its project list.
fn project_id(body: &str, project_dir: &Path) -> Option<String> {
    let json: Value = serde_json::from_str(body).ok()?;
    let projects = json.get("projects")?.as_array()?;
    let wanted = project_dir.canonicalize().ok();
    let id_of = |project: &Value| project.get("id")?.as_str().map(str::to_owned);
    projects
        .iter()
        .find(|project| {
            let dir = project.get("dir").and_then(Value::as_str).map(Path::new);
            wanted.is_some() && dir.and_then(|dir| dir.canonicalize().ok()) == wanted
        })
        .or_else(|| projects.first())
        .and_then(id_of)
}

/// Percent-encodes one path segment.
fn encode_segment(segment: &str) -> String {
    let mut encoded = String::new();
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// The end of the server's own output, where it says what went wrong.
fn tail(log: &Path) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(8)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_200_has_a_body() {
        let ok = "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\r\n{\"a\":1}";
        assert_eq!(parse_response(ok).as_deref(), Some("{\"a\":1}"));
        assert_eq!(parse_response("HTTP/1.1 404 Not Found\r\n\r\nnope"), None);
        assert_eq!(parse_response("garbage"), None);
    }

    #[test]
    fn the_project_is_found_by_its_folder() {
        let dir = tempfile::tempdir().unwrap();
        let mine = dir.path().join("mine");
        let other = dir.path().join("other");
        std::fs::create_dir_all(&mine).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let body = serde_json::json!({ "projects": [
            { "id": "other", "dir": other },
            { "id": "mine", "dir": mine },
        ]})
        .to_string();
        assert_eq!(project_id(&body, &mine).as_deref(), Some("mine"));
    }

    #[test]
    fn a_single_listed_project_is_used_when_folders_do_not_match() {
        let dir = tempfile::tempdir().unwrap();
        let body = r#"{"projects":[{"id":"demo","dir":"/somewhere/else"}]}"#;
        assert_eq!(project_id(body, dir.path()).as_deref(), Some("demo"));
        assert_eq!(project_id(r#"{"projects":[]}"#, dir.path()), None);
    }

    #[test]
    fn ids_are_safe_in_a_url() {
        assert_eq!(encode_segment("launch-film_2.0"), "launch-film_2.0");
        assert_eq!(encode_segment("my film/é"), "my%20film%2F%C3%A9");
    }

    #[test]
    fn only_files_a_composition_is_made_of_move_the_signature() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path();
        let write = |file: &str, text: &str| {
            let path = project.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write("index.html", "<div></div>");
        write("assets/audio/vo.wav", "sound");
        let start = project_signature(project);

        // What Finder, the assistant's checks and an export leave behind.
        for (file, text) in [
            (".DS_Store", "x"),
            ("assets/.DS_Store", "x"),
            ("snapshots/frame-01.png", "x"),
            ("renders/film.mp4", "x"),
            (".hyperframes/backup/b1", "x"),
            ("script.txt", "notes"),
            ("CLAUDE.md", "notes"),
            ("node_modules/pkg/index.js", "x"),
            ("package-lock.json", "{}"),
        ] {
            write(file, text);
            assert_eq!(project_signature(project), start, "{file} should not restart the preview");
        }

        // What does change the film.
        let mut last = start;
        for (file, text) in [
            ("index.html", "<div>longer</div>"),
            ("assets/audio/vo-2.wav", "new sound"),
            ("compositions/intro.html", "<section></section>"),
            ("assets/site/hero.png", "picture"),
        ] {
            write(file, text);
            let now = project_signature(project);
            assert_ne!(now, last, "{file} should restart the preview");
            last = now;
        }
        std::fs::remove_file(project.join("assets/audio/vo-2.wav")).unwrap();
        assert_ne!(project_signature(project), last, "a removed file counts too");
    }

    #[tokio::test]
    async fn a_free_port_can_be_bound() {
        let port = free_port().await.unwrap();
        assert!(tokio::net::TcpListener::bind((HOST, port)).await.is_ok());
    }

    #[tokio::test]
    async fn nothing_listening_is_an_error_not_a_hang() {
        let port = free_port().await.unwrap();
        assert!(http_get(port, "/api/projects").await.is_err());
    }
}
