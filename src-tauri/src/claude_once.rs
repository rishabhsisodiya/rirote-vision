//! One headless Claude Code answer on the user's own plan: no session kept,
//! no user hooks or MCP servers, and only the built-in tools the caller names.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::reference::run::{self, Cancel};
use crate::shell_env;

pub struct Ask<'a> {
    /// Sent on stdin, so a long prompt never meets the argument limit.
    pub prompt: &'a str,
    pub system_prompt: &'a str,
    /// Built-in tools it may use; empty means none at all.
    pub tools: &'a [&'a str],
    /// Where it runs; file tools are confined to it.
    pub dir: &'a Path,
    /// `--model`; empty leaves Claude Code's choice.
    pub model: &'a str,
    pub limit: Duration,
    /// Names the step in messages: "the analysis".
    pub what: &'a str,
}

pub struct Reply {
    pub text: String,
    /// The model that wrote most of the answer.
    pub model: String,
}

pub async fn ask(ask: Ask<'_>, cancel: &Cancel) -> AppResult<Reply> {
    let claude = shell_env::require_program("claude", "Install Claude Code and sign in, then check the setup again.")?;
    let mut command = shell_env::command(&claude);
    let tools = ask.tools.join(",");
    command
        .args(["-p", "--output-format", "json", "--append-system-prompt", ask.system_prompt])
        .args(["--tools", &tools])
        .args(["--strict-mcp-config", "--setting-sources", "project,local", "--no-session-persistence"])
        .current_dir(ask.dir)
        .env("PWD", ask.dir)
        .stdin(std::process::Stdio::piped());
    if !ask.tools.is_empty() {
        command.args(["--allowedTools", &tools]);
    }
    if !ask.model.is_empty() {
        command.args(["--model", ask.model]);
    }
    cancel.check()?;
    command.process_group(0);
    let mut child = command
        .spawn()
        .map_err(|error| AppError::ToolFailed(format!("Could not start Claude Code: {error}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin.write_all(ask.prompt.as_bytes()).await?;
    }
    let output = run::wait(child, ask.limit, ask.what, cancel).await?;
    let envelope: Value = serde_json::from_slice(&output.stdout).map_err(|_| {
        AppError::ToolFailed(format!("Claude Code did not answer.\n{}", shell_env::failure_text(&output)))
    })?;
    if envelope.get("is_error").and_then(Value::as_bool) == Some(true)
        || envelope.get("subtype").and_then(Value::as_str) != Some("success")
    {
        let said = envelope.get("result").and_then(Value::as_str).unwrap_or("no reason given");
        return Err(AppError::ToolFailed(format!("Claude Code could not finish {}: {said}", ask.what)));
    }
    let model = envelope
        .get("modelUsage")
        .and_then(Value::as_object)
        .and_then(|usage| {
            usage
                .iter()
                .max_by_key(|(_, numbers)| numbers.get("outputTokens").and_then(Value::as_u64).unwrap_or(0))
                .map(|(name, _)| name.clone())
        })
        .unwrap_or_else(|| "claude-code".into());
    Ok(Reply { text: envelope.get("result").and_then(Value::as_str).unwrap_or("").to_owned(), model })
}

/// The JSON object in a reply that may have fences or stray words around it.
pub fn json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end > start).then(|| &text[start..=end])
}
