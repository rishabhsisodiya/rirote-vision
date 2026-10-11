//! Runs the assistant's turns: one process per request, its output turned
//! into events for the window, its permission requests answered by policy or
//! by the user, and the conversation saved as it goes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};

use super::chat::{Chat, ChatMessage, ChatStore, ChatView, Trust};
use super::{policy, Engine, PermissionRequest, Rule, Signal, Turn};
use crate::error::{AppError, AppResult};
use crate::{hyperframes, log};

/// Longest request accepted, in characters.
const MAX_PROMPT: usize = 20_000;
/// How much of the agent's error output is kept to explain a failure.
const STDERR_KEPT: usize = 4_000;

/// What the window is told about a turn, as it happens.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AgentEvent {
    TextDelta { text: String },
    Text { text: String },
    Tool { id: String, tool: String, detail: String },
    ToolResult { id: String, ok: bool },
    /// The assistant is waiting for the user to allow or deny something.
    #[serde(rename_all = "camelCase")]
    Approval { request_id: String, tool: String, detail: String, rule: Option<String> },
    Finished { ok: bool, text: String },
    Failed { message: String },
    Stopped,
}

/// Receives each event with the project folder it belongs to.
pub type Listener = Arc<dyn Fn(&Path, AgentEvent) + Send + Sync>;

/// The user's answer to a permission request.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Answer {
    Allow,
    /// Allow, and stop asking about requests the suggested rule covers.
    Always,
    Deny,
}

/// What the editor is showing when a request is sent, so "here" and "this
/// clip" mean something to the assistant.
#[derive(Debug, Default, Deserialize)]
pub struct EditorContext {
    pub playhead: Option<f64>,
    pub selected: Option<SelectedClip>,
}

#[derive(Debug, Deserialize)]
pub struct SelectedClip {
    #[serde(rename = "ref")]
    pub reference: String,
    pub start: f64,
    pub end: f64,
}

type SharedStdin = Arc<tokio::sync::Mutex<Option<ChildStdin>>>;

struct Active {
    turn: u64,
    project: PathBuf,
    /// The agent's process group; None until the process has started.
    pid: Option<i32>,
    stdin: SharedStdin,
    /// Requests shown to the user and not yet answered.
    pending: HashMap<String, PermissionRequest>,
    rules: Vec<Rule>,
    trust: Trust,
    stopped: bool,
}

pub struct AgentService {
    data_dir: PathBuf,
    store: ChatStore,
    engine: Arc<dyn Engine>,
    /// The one turn in progress, if any.
    active: Arc<Mutex<Option<Active>>>,
    turns: AtomicU64,
}

impl AgentService {
    pub fn new(data_dir: &Path, engine: Arc<dyn Engine>) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            store: ChatStore::new(data_dir),
            engine,
            active: Arc::new(Mutex::new(None)),
            turns: AtomicU64::new(1),
        }
    }

    pub fn view(&self, project: &Path) -> ChatView {
        let chat = self.store.load(project);
        let active = lock(&self.active);
        let running = active.as_ref().filter(|turn| turn.project == project);
        ChatView {
            messages: chat.messages,
            busy: running.is_some(),
            rules: running.map(|turn| turn.rules.clone()).unwrap_or(chat.rules),
            trust: chat.trust,
        }
    }

    /// Starts a turn. Its progress arrives through `listener`.
    pub async fn send(
        &self,
        project: &Path,
        prompt: &str,
        context: &EditorContext,
        listener: Listener,
    ) -> AppResult<()> {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Err(AppError::InvalidInput("Type what you want done first.".into()));
        }
        if prompt.chars().count() > MAX_PROMPT {
            return Err(AppError::InvalidInput("That request is too long.".into()));
        }

        self.engine.ready()?;

        let mut chat = self.store.load(project);
        let trust = chat.trust.ok_or_else(|| {
            AppError::Refused("Choose how much the assistant may do in this project first.".into())
        })?;
        let turn = self.turns.fetch_add(1, Ordering::Relaxed);
        let stdin: SharedStdin = Arc::default();
        {
            // Claim the single slot before anything is started.
            let mut active = lock(&self.active);
            if active.is_some() {
                return Err(AppError::Refused("The assistant is still working on the last request.".into()));
            }
            *active = Some(Active {
                turn,
                project: project.to_path_buf(),
                pid: None,
                stdin: stdin.clone(),
                pending: HashMap::new(),
                rules: chat.rules.clone(),
                trust,
                stopped: false,
            });
        }

        let started = self.start(project, &chat, &with_context(prompt, context), &stdin).await;
        let child = match started {
            Ok(child) => child,
            Err(error) => {
                lock(&self.active).take();
                return Err(error);
            }
        };
        if let Some(active) = lock(&self.active).as_mut() {
            active.pid = child.id().and_then(|pid| i32::try_from(pid).ok());
        }

        chat.push(ChatMessage::said("user", prompt));
        self.store.save(project, &chat)?;
        log::write(&self.data_dir, "assistant", &format!("turn {turn} started"));

        let run = Run {
            turn,
            project: project.to_path_buf(),
            data_dir: self.data_dir.clone(),
            store: ChatStore::new(&self.data_dir),
            engine: self.engine.clone(),
            active: self.active.clone(),
            stdin,
            listener,
        };
        tokio::spawn(run.follow(child, chat));
        Ok(())
    }

    async fn start(&self, project: &Path, chat: &Chat, request: &str, stdin: &SharedStdin) -> AppResult<Child> {
        let briefing = briefing(&self.data_dir);
        let turn = Turn {
            project,
            resume: chat.session.as_deref(),
            briefing: &briefing,
            // `send` refuses before this point when no level has been chosen.
            trust: chat.trust.unwrap_or(Trust::Ask),
            rules: &chat.rules,
            data_dir: &self.data_dir,
        };
        let mut command = self.engine.command(&turn)?;
        // Its own process group, so stopping it stops what it started too.
        command.stdin(Stdio::piped()).process_group(0);
        let mut child = command
            .spawn()
            .map_err(|error| AppError::ToolFailed(format!("The assistant could not start: {error}")))?;
        *stdin.lock().await = child.stdin.take();
        write_line(stdin, &self.engine.request_line(request)).await?;
        Ok(child)
    }

    /// Passes the user's answer to a waiting permission request.
    pub async fn answer(&self, request_id: &str, answer: Answer) -> AppResult<()> {
        let (request, stdin) = {
            let mut active = lock(&self.active);
            let turn = active
                .as_mut()
                .ok_or_else(|| AppError::NotFound("That request is no longer waiting.".into()))?;
            let request = turn
                .pending
                .remove(request_id)
                .ok_or_else(|| AppError::NotFound("That request is no longer waiting.".into()))?;
            if answer == Answer::Always {
                if let Some(rule) = request.rule.clone().filter(|rule| !turn.rules.contains(rule)) {
                    turn.rules.push(rule);
                }
            }
            (request, turn.stdin.clone())
        };
        write_line(&stdin, &self.engine.answer_line(&request, answer != Answer::Deny)).await
    }

    /// Stops the running turn. The follower reports it as stopped.
    pub fn stop(&self) {
        let mut active = lock(&self.active);
        let Some(turn) = active.as_mut() else { return };
        turn.stopped = true;
        if let Some(pid) = turn.pid {
            // SAFETY: `pid` is the group this service created for the agent
            // with `process_group(0)`; the signal reaches nothing else.
            unsafe { libc::killpg(pid, libc::SIGTERM) };
        }
    }

    /// Sets how much the assistant may do in this project from now on.
    pub fn set_trust(&self, project: &Path, trust: Trust) -> AppResult<ChatView> {
        self.refuse_while_busy(project)?;
        let mut chat = self.store.load(project);
        chat.trust = Some(trust);
        self.store.save(project, &chat)?;
        log::write(&self.data_dir, "assistant", &format!("trust set to {trust:?}"));
        Ok(self.view(project))
    }

    /// Starts a new conversation. Standing permissions are kept.
    pub fn clear(&self, project: &Path) -> AppResult<ChatView> {
        self.refuse_while_busy(project)?;
        let mut chat = self.store.load(project);
        chat.messages.clear();
        chat.session = None;
        self.store.save(project, &chat)?;
        Ok(self.view(project))
    }

    /// Withdraws every "always allow" given in this project.
    pub fn forget_rules(&self, project: &Path) -> AppResult<ChatView> {
        self.refuse_while_busy(project)?;
        let mut chat = self.store.load(project);
        chat.rules.clear();
        self.store.save(project, &chat)?;
        Ok(self.view(project))
    }

    fn refuse_while_busy(&self, project: &Path) -> AppResult<()> {
        if lock(&self.active).as_ref().is_some_and(|turn| turn.project == project) {
            return Err(AppError::Refused("Wait for the assistant to finish, or stop it first.".into()));
        }
        Ok(())
    }
}

/// Everything the task following one turn needs.
struct Run {
    turn: u64,
    project: PathBuf,
    data_dir: PathBuf,
    store: ChatStore,
    engine: Arc<dyn Engine>,
    active: Arc<Mutex<Option<Active>>>,
    stdin: SharedStdin,
    listener: Listener,
}

impl Run {
    fn emit(&self, event: AgentEvent) {
        (self.listener)(&self.project, event);
    }

    fn save(&self, chat: &Chat) {
        if let Err(error) = self.store.save(&self.project, chat) {
            log::write(&self.data_dir, "assistant", &format!("conversation not saved: {error}"));
        }
    }

    async fn follow(self, mut child: Child, mut chat: Chat) {
        let errors = child.stderr.take().map(|stderr| tokio::spawn(read_tail(stderr)));
        let mut finished = false;
        if let Some(stdout) = child.stdout.take() {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                for signal in self.engine.read(&line) {
                    finished |= self.handle(signal, &mut chat).await;
                }
            }
        }
        let _ = child.wait().await;
        let stderr = match errors {
            Some(task) => task.await.unwrap_or_default(),
            None => String::new(),
        };

        // Free the slot; the rules may have grown during the turn.
        let ended = {
            let mut active = lock(&self.active);
            let ours = active.as_ref().is_some_and(|turn| turn.turn == self.turn);
            if ours { active.take() } else { None }
        };
        let stopped = ended.as_ref().is_some_and(|turn| turn.stopped);
        if let Some(turn) = ended {
            chat.rules = turn.rules;
        }
        chat.close_open_tools();

        if !finished {
            if stopped {
                chat.push(ChatMessage::said("notice", "Stopped."));
                self.emit(AgentEvent::Stopped);
            } else {
                let message = explain_failure(&stderr, &mut chat);
                chat.push(ChatMessage::said("notice", &message));
                self.emit(AgentEvent::Failed { message });
            }
        }
        self.save(&chat);
        log::write(&self.data_dir, "assistant", &format!("turn {} ended", self.turn));
    }

    /// Acts on one signal. True when it ended the turn.
    async fn handle(&self, signal: Signal, chat: &mut Chat) -> bool {
        match signal {
            Signal::Session(session) => {
                chat.session = Some(session);
                self.save(chat);
            }
            Signal::TextDelta(text) => self.emit(AgentEvent::TextDelta { text }),
            Signal::Text(text) => {
                chat.push(ChatMessage::said("assistant", &text));
                self.emit(AgentEvent::Text { text });
            }
            Signal::Tool { id, tool, detail } => {
                let detail = shown_detail(&detail, &self.project);
                chat.push(ChatMessage::tool(&id, &tool, &detail));
                self.emit(AgentEvent::Tool { id, tool, detail });
            }
            Signal::ToolResult { id, ok } => {
                chat.finish_tool(&id, ok);
                self.emit(AgentEvent::ToolResult { id, ok });
            }
            Signal::Permission(request) => self.decide(request).await,
            Signal::Finished { ok, text } => {
                if !ok && !text.is_empty() {
                    chat.push(ChatMessage::said("notice", &text));
                }
                self.save(chat);
                self.emit(AgentEvent::Finished { ok, text });
                // Closing its input tells the agent there is no more to do.
                self.stdin.lock().await.take();
                return true;
            }
        }
        false
    }

    /// Lets the request through when policy allows it; otherwise shows it
    /// to the user and leaves the agent waiting.
    async fn decide(&self, request: PermissionRequest) {
        let (rules, trust) = lock(&self.active)
            .as_ref()
            .map(|turn| (turn.rules.clone(), turn.trust))
            .unwrap_or((Vec::new(), Trust::Ask));
        // At full trust nothing reaches the user. What Claude Code refuses by
        // the user's own deny rules never gets this far.
        if trust == Trust::Full || policy::allowed(&request, &self.project, &rules) {
            let _ = write_line(&self.stdin, &self.engine.answer_line(&request, true)).await;
            return;
        }
        let event = AgentEvent::Approval {
            request_id: request.request_id.clone(),
            tool: request.tool.clone(),
            detail: shown_detail(&request.detail, &self.project),
            rule: request.rule.as_ref().map(|rule| rule.content.clone()).filter(|rule| !rule.is_empty()),
        };
        if let Some(turn) = lock(&self.active).as_mut() {
            turn.pending.insert(request.request_id.clone(), request);
        }
        self.emit(event);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

async fn write_line(stdin: &SharedStdin, line: &str) -> AppResult<()> {
    let mut stdin = stdin.lock().await;
    let pipe = stdin
        .as_mut()
        .ok_or_else(|| AppError::Refused("The assistant has already finished.".into()))?;
    pipe.write_all(line.as_bytes()).await?;
    pipe.write_all(b"\n").await?;
    pipe.flush().await?;
    Ok(())
}

/// The end of the agent's error output.
async fn read_tail(mut stderr: tokio::process::ChildStderr) -> String {
    let mut bytes = Vec::new();
    let _ = stderr.read_to_end(&mut bytes).await;
    let text = String::from_utf8_lossy(&bytes);
    let skip = text.chars().count().saturating_sub(STDERR_KEPT);
    text.chars().skip(skip).collect()
}

/// Why a turn ended without a result, in words for the user.
fn explain_failure(stderr: &str, chat: &mut Chat) -> String {
    if stderr.contains("No conversation found") {
        chat.session = None;
        return "The earlier conversation could not be continued. Send your request again to start a new one.".into();
    }
    let reason = stderr.lines().rev().find(|line| !line.trim().is_empty()).unwrap_or("").trim();
    if reason.is_empty() {
        "The assistant stopped before finishing. Check Setup shows whether Claude Code is signed in.".into()
    } else {
        format!("The assistant stopped before finishing: {reason}")
    }
}

/// A path inside the project is shown relative to it.
fn shown_detail(detail: &str, project: &Path) -> String {
    let relative = Path::new(detail).strip_prefix(project).ok().and_then(Path::to_str);
    relative.filter(|path| !path.is_empty()).unwrap_or(detail).to_owned()
}

/// The request with a line about what the editor is showing in front of it.
fn with_context(prompt: &str, context: &EditorContext) -> String {
    let mut facts = Vec::new();
    if let Some(playhead) = context.playhead.filter(|time| time.is_finite()) {
        facts.push(format!("playhead at {playhead:.2}s"));
    }
    if let Some(clip) = &context.selected {
        let safe: String = clip.reference.chars().filter(|ch| !ch.is_control()).take(120).collect();
        facts.push(format!("selected clip {safe} from {:.2}s to {:.2}s", clip.start, clip.end));
    }
    if facts.is_empty() {
        prompt.to_owned()
    } else {
        format!("[Editor state: {}]\n\n{prompt}", facts.join("; "))
    }
}

/// What the agent is told about where it is running.
fn briefing(data_dir: &Path) -> String {
    let engine = hyperframes::entry(data_dir);
    format!(
        "You are the assistant inside RIROTE VISION, a video editor for macOS built on HyperFrames. \
The current folder is the open project: a HyperFrames composition. The user is looking at a live \
preview and a timeline of this project, and both reload by themselves when files change. \
Do not start a preview server, open a browser or open the HyperFrames desktop app. \
Run the engine as plain `hyperframes` (HyperFrames {version}, already on your PATH) wherever you \
would run `npx hyperframes`, for example `hyperframes timeline --json` or `hyperframes check`; it is \
the app's own copy at {engine}. \
Keep every change inside the project folder, and do not render a video unless asked. \
Requests here are edits to a video, not software features: carry them out directly instead of \
running a multi-phase development workflow, unless the user asks for a plan. \
A message may begin with an [Editor state] line giving the playhead and the selected clip; use it \
to resolve words such as \"here\" and \"this clip\". \
Reply briefly and in plain language: say what changed in the video, not how the code changed.",
        engine = engine.display(),
        version = hyperframes::PINNED_VERSION,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_editor_state_goes_in_front_of_the_request() {
        let none = EditorContext::default();
        assert_eq!(with_context("trim it", &none), "trim it");

        let both = EditorContext {
            playhead: Some(5.2),
            selected: Some(SelectedClip { reference: "#s1\n".into(), start: 3.123, end: 7.4 }),
        };
        assert_eq!(
            with_context("trim it", &both),
            "[Editor state: playhead at 5.20s; selected clip #s1 from 3.12s to 7.40s]\n\ntrim it"
        );
    }

    #[test]
    fn paths_in_the_project_are_shown_short() {
        let project = Path::new("/Users/me/film");
        assert_eq!(shown_detail("/Users/me/film/assets/a.png", project), "assets/a.png");
        assert_eq!(shown_detail("/Users/me/other/a.png", project), "/Users/me/other/a.png");
        assert_eq!(shown_detail("npm run build", project), "npm run build");
        assert_eq!(shown_detail("/Users/me/film", project), "/Users/me/film");
    }

    #[test]
    fn a_lost_conversation_is_forgotten_and_other_failures_are_quoted() {
        let mut chat = Chat { session: Some("old".into()), ..Chat::default() };
        let lost = explain_failure("Error: No conversation found with session ID: old\n", &mut chat);
        assert!(lost.contains("Send your request again"));
        assert_eq!(chat.session, None);

        let mut chat = Chat::default();
        assert!(explain_failure("warning\nInvalid API key\n\n", &mut chat).ends_with("Invalid API key"));
        assert!(explain_failure("", &mut chat).contains("Check Setup"));
    }

    #[test]
    fn events_are_named_for_the_window() {
        let event = AgentEvent::Approval {
            request_id: "r1".into(),
            tool: "Bash".into(),
            detail: "ls".into(),
            rule: Some("ls:*".into()),
        };
        let json = serde_json::to_value(event).unwrap();
        assert_eq!(json["kind"], "approval");
        assert_eq!(json["requestId"], "r1");
        assert_eq!(serde_json::to_value(AgentEvent::Stopped).unwrap()["kind"], "stopped");
    }
}
