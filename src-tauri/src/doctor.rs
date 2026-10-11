//! Check Setup: what the app needs on this Mac, and what is missing.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{hyperframes, shell_env, skills};

const MIN_NODE_MAJOR: u32 = 22;
const QUICK_TIMEOUT: Duration = Duration::from_secs(15);
const ENGINE_CHECK_TIMEOUT: Duration = Duration::from_secs(60);

/// The interface offers a button for a check that carries this action.
const ACTION_INSTALL_ENGINE: &str = "install-engine";
const ACTION_INSTALL_VOICE: &str = "install-voice";
const ACTION_INSTALL_SKILLS: &str = "install-skills";

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Warning,
    Missing,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub id: &'static str,
    pub label: &'static str,
    pub status: Status,
    pub detail: String,
    pub hint: Option<String>,
    /// The app cannot edit or export without this.
    pub required: bool,
    pub action: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    /// Every required check passed.
    pub ready: bool,
    pub checks: Vec<Check>,
}

impl Report {
    fn new(checks: Vec<Check>) -> Self {
        let ready = checks.iter().all(|check| !check.required || check.status == Status::Ok);
        Self { ready, checks }
    }
}

pub async fn run(data_dir: &Path) -> Report {
    let (claude, node, mut engine) =
        tokio::join!(check_claude(), check_node(), check_engine(data_dir));
    let node_ok = node.status == Status::Ok;
    let engine_ok = engine.first().is_some_and(|check| check.status == Status::Ok);
    let mut checks = vec![claude, node, check_skills(&skills::folder(), engine_ok)];
    if !node_ok {
        if let Some(first) = engine.first_mut().filter(|check| check.status != Status::Ok) {
            first.action = None;
            first.hint = Some("Install Node.js first, then install the video engine.".into());
        }
    }
    checks.append(&mut engine);
    Report::new(checks)
}

async fn check_claude() -> Check {
    let check = |status, detail: &str, hint: Option<&str>| Check {
        id: "claude",
        label: "Claude Code",
        status,
        detail: detail.to_owned(),
        hint: hint.map(str::to_owned),
        required: true,
        action: None,
    };
    let Some(claude) = shell_env::find_program("claude") else {
        return check(
            Status::Missing,
            "Not installed",
            Some("Install Claude Code, then sign in with your Claude plan."),
        );
    };
    let mut command = shell_env::command(&claude);
    command.args(["auth", "status"]);
    let Ok(output) = shell_env::run(command, QUICK_TIMEOUT, "claude auth status").await else {
        return check(Status::Warning, "Installed, but the sign-in status could not be read", None);
    };
    match claude_sign_in(&String::from_utf8_lossy(&output.stdout)) {
        SignIn::Plan(plan) => check(Status::Ok, &format!("Signed in · {plan}"), None),
        SignIn::ApiKey => check(
            Status::Warning,
            "Signed in with an API key",
            Some("Usage is billed to that key, not to a Claude plan. Run `claude` in Terminal and use /login to switch."),
        ),
        SignIn::SignedOut => check(
            Status::Missing,
            "Not signed in",
            Some("Open Terminal, run `claude` and sign in with your Claude plan."),
        ),
        SignIn::Unknown => {
            check(Status::Warning, "Installed, but the sign-in status could not be read", None)
        }
    }
}

/// The assistant's instructions for working with HyperFrames.
fn check_skills(folder: &std::path::Path, engine_ok: bool) -> Check {
    let missing = skills::missing(folder);
    let total = skills::core_count();
    let ok = missing.is_empty();
    Check {
        id: "skills",
        label: "Video skills",
        status: if ok { Status::Ok } else { Status::Missing },
        detail: if ok {
            format!("All {total} core skills are in {}", folder.display())
        } else {
            format!("{} of {total} core skills are missing", missing.len())
        },
        hint: if ok {
            None
        } else if engine_ok {
            Some(format!(
                "HyperFrames' instructions for the assistant. They are downloaded into {}, where Claude Code in other apps can use them too.",
                folder.display()
            ))
        } else {
            Some("Install the video engine first; it installs the skills.".into())
        },
        required: true,
        // The engine carries the installer, so it has to be there first.
        action: (!ok && engine_ok).then_some(ACTION_INSTALL_SKILLS),
    }
}

#[derive(Debug, PartialEq)]
enum SignIn {
    Plan(String),
    ApiKey,
    SignedOut,
    Unknown,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthStatus {
    logged_in: bool,
    auth_method: Option<String>,
    subscription_type: Option<String>,
}

fn claude_sign_in(stdout: &str) -> SignIn {
    let Ok(status) = serde_json::from_str::<AuthStatus>(stdout.trim()) else {
        return SignIn::Unknown;
    };
    if !status.logged_in {
        return SignIn::SignedOut;
    }
    if status.auth_method.as_deref() != Some("claude.ai") {
        return SignIn::ApiKey;
    }
    let plan = match status.subscription_type.as_deref() {
        Some("pro") => "Claude Pro plan",
        Some("max") => "Claude Max plan",
        Some("team") => "Claude Team plan",
        Some("enterprise") => "Claude Enterprise plan",
        _ => "Claude plan",
    };
    SignIn::Plan(plan.to_owned())
}

async fn check_node() -> Check {
    let check = |status, detail: String, hint: Option<String>| Check {
        id: "node",
        label: "Node.js",
        status,
        detail,
        hint,
        required: true,
        action: None,
    };
    let install_hint = format!("Install Node.js {MIN_NODE_MAJOR} or newer from nodejs.org.");
    let Some(node) = shell_env::find_program("node") else {
        return check(Status::Missing, "Not installed".into(), Some(install_hint));
    };
    let mut command = shell_env::command(&node);
    command.arg("--version");
    let version = match shell_env::run(command, QUICK_TIMEOUT, "node").await {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        Err(error) => return check(Status::Missing, error.to_string(), Some(install_hint)),
    };
    match node_major(&version) {
        Some(major) if major >= MIN_NODE_MAJOR => check(Status::Ok, version, None),
        _ => check(Status::Missing, format!("{version} is too old"), Some(install_hint)),
    }
}

fn node_major(version: &str) -> Option<u32> {
    version.trim().trim_start_matches('v').split('.').next()?.parse().ok()
}

/// The engine itself, then the tools the engine says it needs.
async fn check_engine(data_dir: &Path) -> Vec<Check> {
    let pinned = hyperframes::PINNED_VERSION;
    let engine = |status, detail: String| Check {
        id: "engine",
        label: "Video engine",
        status,
        detail,
        hint: (status != Status::Ok)
            .then(|| "A one-time download of about 120 MB into the app's own folder.".to_owned()),
        required: true,
        action: (status != Status::Ok).then_some(ACTION_INSTALL_ENGINE),
    };
    match hyperframes::installed_version(data_dir) {
        Some(version) if version == pinned => {}
        Some(version) => {
            return vec![engine(
                Status::Missing,
                format!("HyperFrames {version} is installed; this app needs {pinned}"),
            )]
        }
        None => return vec![engine(Status::Missing, "Not installed".into())],
    }

    let mut checks = vec![engine(Status::Ok, format!("HyperFrames {pinned}"))];
    let cwd = data_dir.to_path_buf();
    match hyperframes::run_json::<Value>(data_dir, &["doctor", "--json"], &cwd, ENGINE_CHECK_TIMEOUT)
        .await
    {
        Ok(report) => checks.extend(engine_tools(&report)),
        Err(error) => checks.push(Check {
            id: "engine-tools",
            label: "Engine self-check",
            status: Status::Warning,
            detail: error.to_string(),
            hint: None,
            required: false,
            action: None,
        }),
    }
    checks
}

/// The engine checks many things; these are the ones that decide whether
/// editing, export, captions and voiceover work.
const ENGINE_TOOLS: [(&str, &str, &str, bool); 4] = [
    ("FFmpeg", "ffmpeg", "FFmpeg", true),
    ("Chrome", "chrome", "Render browser", true),
    ("whisper-cpp", "whisper", "Captions (Whisper)", false),
    ("TTS (Kokoro)", "kokoro", "Voiceover and music", false),
];

fn engine_tools(report: &Value) -> Vec<Check> {
    let reported = report.get("checks").and_then(Value::as_array).cloned().unwrap_or_default();
    ENGINE_TOOLS
        .iter()
        .filter_map(|&(name, id, label, required)| {
            let found = reported.iter().find(|check| check.get("name").and_then(Value::as_str) == Some(name))?;
            let text = |key: &str| found.get(key).and_then(Value::as_str).map(str::to_owned);
            let ok = found.get("ok").and_then(Value::as_bool).unwrap_or(false);
            let status = match (ok, required) {
                (true, _) => Status::Ok,
                (false, true) => Status::Missing,
                (false, false) => Status::Warning,
            };
            // Voiceover and music are repaired by the app itself, not by a pip command.
            let voice = id == "kokoro" && !ok;
            Some(Check {
                id,
                label,
                status,
                detail: text("detail").unwrap_or_default(),
                hint: if voice {
                    Some("A one-time download into the app's own folder. Needs Python 3.10 to 3.13 on this Mac.".into())
                } else {
                    text("hint")
                },
                required,
                action: voice.then_some(ACTION_INSTALL_VOICE),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_plan_login_names_the_plan() {
        let stdout = r#"{"loggedIn":true,"authMethod":"claude.ai","subscriptionType":"pro"}"#;
        assert_eq!(claude_sign_in(stdout), SignIn::Plan("Claude Pro plan".into()));
    }

    #[test]
    fn an_api_key_login_is_told_apart() {
        let stdout = r#"{"loggedIn":true,"authMethod":"api_key"}"#;
        assert_eq!(claude_sign_in(stdout), SignIn::ApiKey);
    }

    #[test]
    fn signed_out_and_unreadable_answers() {
        assert_eq!(claude_sign_in(r#"{"loggedIn":false}"#), SignIn::SignedOut);
        assert_eq!(claude_sign_in("Not logged in"), SignIn::Unknown);
    }

    #[test]
    fn missing_skills_can_be_installed_once_the_engine_is_there() {
        let dir = tempfile::tempdir().unwrap();
        let before = check_skills(dir.path(), false);
        assert_eq!((before.status, before.action, before.required), (Status::Missing, None, true));
        assert!(before.hint.unwrap().contains("video engine first"));

        let ready = check_skills(dir.path(), true);
        assert_eq!(ready.action, Some("install-skills"));
        assert!(ready.detail.starts_with("10 of 10"));
    }

    #[test]
    fn node_versions() {
        assert_eq!(node_major("v24.0.2"), Some(24));
        assert_eq!(node_major("18.19.0\n"), Some(18));
        assert_eq!(node_major("nonsense"), None);
    }

    #[test]
    fn engine_tools_keep_required_apart_from_optional() {
        let report = json!({ "checks": [
            { "name": "FFmpeg", "ok": false, "detail": "Not found", "hint": "brew install ffmpeg" },
            { "name": "whisper-cpp", "ok": false, "detail": "Not found" },
            { "name": "Chrome", "ok": true, "detail": "cached" },
            { "name": "Docker", "ok": false, "detail": "Not running" }
        ]});
        let checks = engine_tools(&report);
        let status = |id: &str| checks.iter().find(|check| check.id == id).map(|check| check.status);
        assert_eq!(status("ffmpeg"), Some(Status::Missing));
        assert_eq!(status("whisper"), Some(Status::Warning));
        assert_eq!(status("chrome"), Some(Status::Ok));
        assert_eq!(checks.len(), 3);
    }

    #[test]
    fn ready_only_when_every_required_check_passes() {
        let check = |status, required| Check {
            id: "x",
            label: "x",
            status,
            detail: String::new(),
            hint: None,
            required,
            action: None,
        };
        assert!(Report::new(vec![check(Status::Ok, true), check(Status::Warning, false)]).ready);
        assert!(!Report::new(vec![check(Status::Warning, true)]).ready);
        assert!(!Report::new(vec![check(Status::Missing, true)]).ready);
    }
}
