//! Claude Code as the agent, run headless with streamed JSON in both
//! directions. It uses the login Claude Code already has, so the user's
//! Claude plan pays for it and the app stores no key.

use serde_json::{json, Value};
use tokio::process::Command;

use super::{Engine, PermissionRequest, Rule, Signal, Trust, Turn};
use crate::error::AppResult;
use crate::{hyperframes, shell_env, skills, voice};

const INSTALL_HINT: &str = "Install Claude Code and sign in with your Claude plan.";

/// What "trusted video work" lets run without asking, as Claude Code allow
/// rules. Claude Code takes a chained command apart and checks each piece,
/// so `cd project && hyperframes check` passes and `hyperframes check && curl …`
/// still asks. The list is short on purpose: these are trusted by name, not
/// by what they touch.
const TRUSTED_TOOLS: [&str; 5] = [
    "Bash(hyperframes:*)",
    "Bash(npx hyperframes:*)",
    "Bash(npm run:*)",
    "Bash(ffmpeg:*)",
    "Bash(ffprobe:*)",
];
const MAX_RULE: usize = 300;

#[derive(Default)]
pub struct ClaudeCode {
    /// A model to use instead of the user's default.
    pub model: Option<String>,
}

impl Engine for ClaudeCode {
    fn ready(&self) -> AppResult<()> {
        // Claude Code without HyperFrames' skills has no idea how a
        // composition works; better to say so than to let it guess.
        skills::require(&skills::folder())
    }

    fn command(&self, turn: &Turn) -> AppResult<Command> {
        let claude = shell_env::require_program("claude", INSTALL_HINT)?;
        let tools = tools_path(turn.data_dir)?;
        let mut command = shell_env::command(&claude);
        command
            .current_dir(turn.project)
            .env("PWD", turn.project)
            .env("PATH", tools)
            .env("HYPERFRAMES_NO_TELEMETRY", "1")
            .env("HYPERFRAMES_NO_UPDATE_CHECK", "1")
            .args(["--print", "--verbose"])
            .args(["--input-format", "stream-json", "--output-format", "stream-json"])
            .arg("--include-partial-messages")
            // Stated outright so a run never starts in a mode that approves
            // everything by itself; requests come to the app over stdin/stdout.
            .args(["--permission-mode", permission_mode(turn.trust), "--permission-prompt-tool", "stdio"])
            .args(["--append-system-prompt", turn.briefing])
            // Left over when the app is started from a terminal session; the
            // run must not claim to be that session.
            .env_remove("CLAUDECODE")
            .env_remove("CLAUDE_CODE_ENTRYPOINT")
            .env_remove("CLAUDE_CODE_SESSION_ID");
        // Engine commands the assistant runs use the app's voice environment too.
        if let Some(python) = voice::python(turn.data_dir) {
            command.env("HYPERFRAMES_PYTHON", python);
        }
        let allowed = allowed_tools(turn.trust, turn.rules);
        if !allowed.is_empty() {
            command.arg("--allowedTools").args(allowed);
        }
        if let Some(session) = turn.resume {
            command.args(["--resume", session]);
        }
        if let Some(model) = &self.model {
            command.args(["--model", model]);
        }
        Ok(command)
    }

    fn request_line(&self, prompt: &str) -> String {
        json!({
            "type": "user",
            "message": { "role": "user", "content": [{ "type": "text", "text": prompt }] },
        })
        .to_string()
    }

    fn read(&self, line: &str) -> Vec<Signal> {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            return Vec::new();
        };
        // What a sub-agent says is its own business; only its tool calls,
        // which need permission like any other, are of interest.
        let nested = !event["parent_tool_use_id"].is_null();
        match event["type"].as_str() {
            Some("system") if event["subtype"] == "init" => {
                text(&event["session_id"]).map(Signal::Session).into_iter().collect()
            }
            Some("stream_event") if !nested => {
                let delta = &event["event"]["delta"];
                match (event["event"]["type"].as_str(), delta["type"].as_str()) {
                    (Some("content_block_delta"), Some("text_delta")) => {
                        text(&delta["text"]).map(Signal::TextDelta).into_iter().collect()
                    }
                    _ => Vec::new(),
                }
            }
            Some("assistant") if !nested => blocks(&event)
                .filter_map(|block| match block["type"].as_str() {
                    Some("text") => text(&block["text"])
                        .filter(|said| !said.trim().is_empty())
                        .map(|said| Signal::Text(said.trim().to_owned())),
                    Some("tool_use") => Some(Signal::Tool {
                        id: text(&block["id"])?,
                        tool: text(&block["name"])?,
                        detail: tool_detail(&block["input"]),
                    }),
                    _ => None,
                })
                .collect(),
            Some("user") if !nested => blocks(&event)
                .filter(|block| block["type"] == "tool_result")
                .filter_map(|block| {
                    Some(Signal::ToolResult {
                        id: text(&block["tool_use_id"])?,
                        ok: !block["is_error"].as_bool().unwrap_or(false),
                    })
                })
                .collect(),
            Some("control_request") if event["request"]["subtype"] == "can_use_tool" => {
                permission(&event).map(Signal::Permission).into_iter().collect()
            }
            Some("result") => vec![Signal::Finished {
                ok: !event["is_error"].as_bool().unwrap_or(false),
                text: event["result"].as_str().unwrap_or("").trim().to_owned(),
            }],
            _ => Vec::new(),
        }
    }

    fn answer_line(&self, request: &PermissionRequest, allow: bool) -> String {
        let decision = if allow {
            json!({ "behavior": "allow", "updatedInput": request.input })
        } else {
            json!({ "behavior": "deny", "message": "The user declined this in RIROTE VISION." })
        };
        json!({
            "type": "control_response",
            "response": { "subtype": "success", "request_id": request.request_id, "response": decision },
        })
        .to_string()
    }
}

/// Claude Code's mode for a trust level. "acceptEdits" lets it create, copy,
/// move and delete files inside the project folder without asking, and
/// still ask for anything outside it. Full trust is the same mode: the rest
/// is answered by the app, so the user's own deny rules keep working.
fn permission_mode(trust: Trust) -> &'static str {
    match trust {
        Trust::Ask => "default",
        Trust::Trusted | Trust::Full => "acceptEdits",
    }
}

/// The allow rules a turn starts with: the fixed list for the level, then
/// what the user chose to always allow in this project.
fn allowed_tools(trust: Trust, rules: &[Rule]) -> Vec<String> {
    let mut allowed: Vec<String> = match trust {
        Trust::Ask => Vec::new(),
        Trust::Trusted | Trust::Full => TRUSTED_TOOLS.iter().map(|rule| (*rule).to_owned()).collect(),
    };
    for rule in rules {
        if let Some(text) = rule_text(rule) {
            if !allowed.contains(&text) {
                allowed.push(text);
            }
        }
    }
    allowed
}

/// A saved rule in Claude Code's own notation, or None for one that could
/// not be passed on a command line safely.
fn rule_text(rule: &Rule) -> Option<String> {
    let plain = |text: &str| {
        !text.is_empty() && text.len() <= MAX_RULE && !text.chars().any(|ch| ch.is_control() || ch == '(' || ch == ')')
    };
    let name = rule.tool.as_str();
    if !plain(name) || name.starts_with('-') || name.contains(char::is_whitespace) {
        return None;
    }
    match rule.content.as_str() {
        "" => Some(name.to_owned()),
        content if plain(content) => Some(format!("{name}({content})")),
        _ => None,
    }
}

/// A `hyperframes` command that runs the app's pinned engine, put first on
/// the path the assistant searches. A plain name is something a permission
/// rule can match; the engine's real path, with its spaces, is not.
fn tools_path(data_dir: &std::path::Path) -> AppResult<std::ffi::OsString> {
    use std::os::unix::fs::PermissionsExt;

    let bin = data_dir.join("bin");
    std::fs::create_dir_all(&bin)?;
    let launcher = bin.join("hyperframes");
    let entry = hyperframes::entry(data_dir).to_string_lossy().replace('\'', r"'\''");
    // Written afresh each time, so an edited copy cannot linger.
    std::fs::write(&launcher, format!("#!/bin/sh\nexec node '{entry}' \"$@\"\n"))?;
    std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755))?;

    let mut dirs = vec![bin];
    dirs.extend(std::env::split_paths(shell_env::search_path()));
    Ok(std::env::join_paths(dirs).unwrap_or_else(|_| shell_env::search_path().clone()))
}

fn text(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

fn blocks(event: &Value) -> impl Iterator<Item = &Value> {
    event["message"]["content"].as_array().into_iter().flatten()
}

fn permission(event: &Value) -> Option<PermissionRequest> {
    let request = &event["request"];
    let input = request["input"].clone();
    let detail = match tool_detail(&input) {
        detail if detail.is_empty() => request["description"].as_str().unwrap_or("").to_owned(),
        detail => detail,
    };
    Some(PermissionRequest {
        request_id: text(&event["request_id"])?,
        tool: text(&request["tool_name"])?,
        detail,
        rule: suggested_rule(&request["permission_suggestions"]),
        input,
    })
}

/// The first rule Claude Code itself proposes for "don't ask again".
fn suggested_rule(suggestions: &Value) -> Option<Rule> {
    suggestions
        .as_array()?
        .iter()
        .filter(|suggestion| suggestion["type"] == "addRules" && suggestion["behavior"] == "allow")
        .find_map(|suggestion| {
            let rule = suggestion["rules"].as_array()?.first()?;
            Some(Rule {
                tool: text(&rule["toolName"])?,
                content: rule["ruleContent"].as_str().unwrap_or("").to_owned(),
            })
        })
}

/// A tool call described by the most specific thing it carries.
fn tool_detail(input: &Value) -> String {
    ["command", "file_path", "notebook_path", "path", "pattern", "url", "query", "skill", "description"]
        .iter()
        .find_map(|key| input[*key].as_str())
        .map(|found| found.trim().to_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(line: &str) -> Vec<Signal> {
        ClaudeCode::default().read(line)
    }

    #[test]
    fn each_trust_level_starts_claude_code_in_its_mode_with_its_rules() {
        assert_eq!(permission_mode(Trust::Ask), "default");
        assert_eq!(permission_mode(Trust::Trusted), "acceptEdits");
        assert_eq!(permission_mode(Trust::Full), "acceptEdits");

        assert!(allowed_tools(Trust::Ask, &[]).is_empty());
        let trusted = allowed_tools(Trust::Trusted, &[]);
        assert!(trusted.contains(&"Bash(hyperframes:*)".to_owned()) && trusted.contains(&"Bash(npm run:*)".to_owned()));
        assert!(!trusted.iter().any(|rule| rule.contains("rm") || rule.contains("curl") || rule.contains("git")));
        assert_eq!(allowed_tools(Trust::Full, &[]), trusted);
    }

    #[test]
    fn always_allow_rules_are_passed_on_at_every_level_when_they_are_plain() {
        let rule = |tool: &str, content: &str| Rule { tool: tool.into(), content: content.into() };
        let saved = [
            rule("Bash", "npm run *"),
            rule("WebFetch", ""),
            rule("Bash", "npm run *"),          // a repeat
            rule("Bash", "echo $(cat secret)"), // brackets: not passed on
            rule("--dangerously", "x"),         // not a tool name
            rule("Bash", "line\nbreak"),
        ];
        assert_eq!(allowed_tools(Trust::Ask, &saved), ["Bash(npm run *)", "WebFetch"]);
        let trusted = allowed_tools(Trust::Trusted, &saved);
        assert_eq!(&trusted[TRUSTED_TOOLS.len()..], ["Bash(npm run *)", "WebFetch"]);
    }

    #[test]
    fn the_assistant_gets_a_plain_engine_command_ahead_of_any_other() {
        use std::os::unix::fs::PermissionsExt;

        let data = tempfile::tempdir().unwrap();
        let path = tools_path(data.path()).unwrap();
        let first = std::env::split_paths(&path).next().unwrap();
        assert_eq!(first, data.path().join("bin"));
        let script = std::fs::read_to_string(first.join("hyperframes")).unwrap();
        assert!(script.starts_with("#!/bin/sh\nexec node '"));
        assert!(script.trim_end().ends_with("hyperframes.mjs' \"$@\""));
        assert_eq!(std::fs::metadata(first.join("hyperframes")).unwrap().permissions().mode() & 0o111, 0o111);
    }

    #[test]
    fn the_first_line_names_the_conversation() {
        let line = r#"{"type":"system","subtype":"init","session_id":"abc-123","tools":[]}"#;
        assert_eq!(read(line), [Signal::Session("abc-123".into())]);
    }

    #[test]
    fn a_reply_arrives_in_pieces_and_then_whole() {
        let piece = r#"{"type":"stream_event","parent_tool_use_id":null,"event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Do"}}}"#;
        assert_eq!(read(piece), [Signal::TextDelta("Do".into())]);
        let thinking = r#"{"type":"stream_event","parent_tool_use_id":null,"event":{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"hm"}}}"#;
        assert!(read(thinking).is_empty());

        let whole = r#"{"type":"assistant","parent_tool_use_id":null,"message":{"content":[
            {"type":"thinking","thinking":""},{"type":"text","text":" Done. "},{"type":"text","text":"  "},
            {"type":"tool_use","id":"t1","name":"Bash","input":{"command":" ls ","description":"List"}}]}}"#;
        assert_eq!(
            read(&whole.replace('\n', "")),
            [
                Signal::Text("Done.".into()),
                Signal::Tool { id: "t1".into(), tool: "Bash".into(), detail: "ls".into() }
            ]
        );
    }

    #[test]
    fn tool_results_say_whether_the_tool_worked() {
        let line = r#"{"type":"user","parent_tool_use_id":null,"message":{"content":[
            {"type":"tool_result","tool_use_id":"t1","content":"ok"},
            {"type":"tool_result","tool_use_id":"t2","is_error":true,"content":"no"}]}}"#;
        assert_eq!(
            read(&line.replace('\n', "")),
            [
                Signal::ToolResult { id: "t1".into(), ok: true },
                Signal::ToolResult { id: "t2".into(), ok: false }
            ]
        );
    }

    #[test]
    fn a_permission_request_carries_what_is_asked_and_a_rule_for_next_time() {
        let line = r#"{"type":"control_request","request_id":"r1","request":{"subtype":"can_use_tool","tool_name":"Bash",
            "input":{"command":"npm run build","description":"Build"},"description":"Build",
            "permission_suggestions":[{"type":"setMode","mode":"acceptEdits"},
              {"type":"addRules","behavior":"allow","rules":[{"toolName":"Bash","ruleContent":"npm run build:*"}]}]}}"#;
        let signals = read(&line.replace('\n', ""));
        let [Signal::Permission(request)] = signals.as_slice() else { panic!("{signals:?}") };
        assert_eq!(request.request_id, "r1");
        assert_eq!(request.detail, "npm run build");
        assert_eq!(request.rule, Some(Rule { tool: "Bash".into(), content: "npm run build:*".into() }));

        let engine = ClaudeCode::default();
        let allow: Value = serde_json::from_str(&engine.answer_line(request, true)).unwrap();
        assert_eq!(allow["response"]["request_id"], "r1");
        assert_eq!(allow["response"]["response"]["behavior"], "allow");
        assert_eq!(allow["response"]["response"]["updatedInput"]["command"], "npm run build");
        let deny: Value = serde_json::from_str(&engine.answer_line(request, false)).unwrap();
        assert_eq!(deny["response"]["response"]["behavior"], "deny");
    }

    #[test]
    fn the_result_ends_the_turn() {
        let ok = r#"{"type":"result","subtype":"success","is_error":false,"result":" All set. "}"#;
        assert_eq!(read(ok), [Signal::Finished { ok: true, text: "All set.".into() }]);
        let failed = r#"{"type":"result","is_error":true,"result":"Not logged in"}"#;
        assert_eq!(read(failed), [Signal::Finished { ok: false, text: "Not logged in".into() }]);
    }

    #[test]
    fn a_sub_agents_chatter_and_everything_else_is_ignored() {
        for line in [
            "",
            "not json",
            r#"{"type":"rate_limit_event"}"#,
            r#"{"type":"system","subtype":"status","status":"requesting"}"#,
            r#"{"type":"assistant","parent_tool_use_id":"t9","message":{"content":[{"type":"text","text":"inner"}]}}"#,
            r#"{"type":"control_request","request_id":"r2","request":{"subtype":"something_else"}}"#,
        ] {
            assert!(read(line).is_empty(), "{line}");
        }
    }

    #[test]
    fn the_request_line_is_one_json_message() {
        let line = ClaudeCode::default().request_line("cut \"this\"\nplease");
        assert!(!line.contains('\n'));
        let json: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(json["message"]["content"][0]["text"], "cut \"this\"\nplease");
    }
}
