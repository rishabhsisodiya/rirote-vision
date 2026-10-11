//! The assistant: an AI coding agent run on the open project, one request
//! (a "turn") at a time, with what it says and does reported as it happens.
//!
//! The agent is an external program the user already has and is signed in
//! to. `Engine` is everything that differs between such programs: how to
//! start one, how to read its output, and how to answer when it asks for
//! permission. The service in `service.rs` owns the rest and is the same for
//! every engine.

mod chat;
mod claude;
mod policy;
mod service;

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;

use crate::error::AppResult;

pub use chat::{ChatView, Trust};
pub use claude::ClaudeCode;
pub use service::{AgentEvent, AgentService, Answer, EditorContext};

/// One request to the agent.
pub struct Turn<'a> {
    pub project: &'a Path,
    /// The conversation to continue, if there is one.
    pub resume: Option<&'a str>,
    /// What the agent is told about where it is running.
    pub briefing: &'a str,
    /// How much it may do without asking.
    pub trust: Trust,
    /// Standing permissions the user gave in this project.
    pub rules: &'a [Rule],
    /// The app's own folder, where the tools it hands the agent live.
    pub data_dir: &'a Path,
}

/// Something the agent's output said, in terms the app understands.
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    /// The id under which this conversation can be continued later.
    Session(String),
    /// A few more characters of the reply being written.
    TextDelta(String),
    /// A finished piece of the reply.
    Text(String),
    Tool { id: String, tool: String, detail: String },
    ToolResult { id: String, ok: bool },
    /// The agent wants to do something and is waiting to be told it may.
    Permission(PermissionRequest),
    Finished { ok: bool, text: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PermissionRequest {
    pub request_id: String,
    pub tool: String,
    pub input: Value,
    pub detail: String,
    /// A rule that would cover this and similar requests from now on.
    pub rule: Option<Rule>,
}

/// A standing permission the user gave: this tool, for this content. For a
/// shell command, content ending in ` *` or `:*` covers every command that
/// starts with what comes before it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub tool: String,
    pub content: String,
}

pub trait Engine: Send + Sync {
    /// Refuses, saying what to do, when the engine lacks something it needs
    /// to work on video at all.
    fn ready(&self) -> AppResult<()>;
    /// The process that carries out a turn, ready to spawn.
    fn command(&self, turn: &Turn) -> AppResult<Command>;
    /// The line that hands the agent the user's request.
    fn request_line(&self, prompt: &str) -> String;
    /// What one line of the agent's output means.
    fn read(&self, line: &str) -> Vec<Signal>;
    /// The line that answers a permission request.
    fn answer_line(&self, request: &PermissionRequest, allow: bool) -> String;
}
