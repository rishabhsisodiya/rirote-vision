//! Shell commands that only look at the project: `cat`, `head`, `tail`,
//! `ls`, `wc` and `stat` on files inside it, with `cd`, `echo` and `pwd` to
//! join them. At "Trusted video work" these run without asking.
//!
//! The check is deliberately narrow. The command is split into words the way
//! the shell would, and anything the shell could turn into a different
//! command (variables, `$(…)`, backticks, backslashes, braces, `~`, writing
//! redirects) makes it ask instead. `;`, `&&`, `||` and `|` may join pieces,
//! but only when every piece passes on its own.
//!
//! Relative paths are read from the project folder, where the assistant
//! starts, or from the folder an earlier `cd` in the same command moved to.

use std::path::{Path, PathBuf};

use super::policy::is_within;

/// Programs that only read the paths they are given.
const READERS: [&str; 6] = ["cat", "head", "tail", "ls", "wc", "stat"];
/// Programs that read no files at all.
const HARMLESS: [&str; 3] = ["echo", "pwd", "true"];
/// Outside quotes, any of these can change what the shell runs or touches.
const UNSAFE: [char; 13] = ['$', '`', '\\', '(', ')', '<', '{', '}', '~', '!', '#', '[', ']'];

/// True when `command` only reads files inside `project`.
pub fn only_reads(command: &str, project: &Path) -> bool {
    let Some(pieces) = split(command) else {
        return false;
    };
    let mut here = project.to_path_buf();
    let mut ran = false;
    for words in pieces.iter().filter(|words| !words.is_empty()) {
        if !piece_reads(words, project, &mut here) {
            return false;
        }
        ran = true;
    }
    ran
}

fn piece_reads(words: &[String], project: &Path, here: &mut PathBuf) -> bool {
    let (program, args) = (words[0].as_str(), &words[1..]);
    if program == "cd" {
        let [target] = args else { return false };
        let next = place(here, target);
        if !is_within(&next, project) {
            return false;
        }
        *here = next;
        return true;
    }
    if HARMLESS.contains(&program) {
        return true;
    }
    if !READERS.contains(&program) {
        return false;
    }
    args.iter().all(|arg| match arg.strip_prefix('-') {
        // Plain flags only: `-n`, `-80`, `-la`. `--x=/path` and the like ask.
        // `tail -f` would never finish, so it asks too.
        Some(flag) => {
            flag.chars().all(|c| c.is_ascii_alphanumeric()) && !(program == "tail" && flag.contains(['f', 'F']))
        }
        // Anything else is taken as a path, even a count like the `80` in
        // `head -n 80`: a path inside the project is harmless either way.
        None => is_within(&place(here, arg), project),
    })
}

fn place(here: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        here.join(path)
    }
}

/// The command's pieces, each as its words, or `None` when the shell could
/// read it as something other than plain words.
fn split(command: &str) -> Option<Vec<Vec<String>>> {
    let mut pieces = vec![Vec::new()];
    let mut word = String::new();
    let mut started = false;
    let mut chars = command.chars().peekable();

    fn finish(pieces: &mut [Vec<String>], word: &mut String, started: &mut bool) {
        if *started {
            pieces.last_mut().expect("there is always a piece").push(std::mem::take(word));
            *started = false;
        }
    }

    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' => finish(&mut pieces, &mut word, &mut started),
            ';' | '\n' | '\r' | '|' | '&' => {
                if c == '&' && chars.next_if_eq(&'&').is_none() {
                    // A lone `&` runs something in the background.
                    return None;
                }
                if c == '|' {
                    chars.next_if_eq(&'|');
                }
                finish(&mut pieces, &mut word, &mut started);
                pieces.push(Vec::new());
            }
            '\'' => {
                started = true;
                loop {
                    match chars.next()? {
                        '\'' => break,
                        inner => word.push(inner),
                    }
                }
            }
            '"' => {
                started = true;
                loop {
                    match chars.next()? {
                        '"' => break,
                        '$' | '`' | '\\' => return None,
                        inner => word.push(inner),
                    }
                }
            }
            '>' => {
                // Only throwing output away is allowed: `2>/dev/null`,
                // `>/dev/null` and `2>&1`. Any other redirect writes a file.
                let descriptor = if started { std::mem::take(&mut word) } else { String::new() };
                started = false;
                if !(descriptor.is_empty() || descriptor == "1" || descriptor == "2") {
                    return None;
                }
                let mut target = String::new();
                while let Some(next) = chars.next_if(|next| !matches!(next, ' ' | '\t' | ';' | '\n' | '\r' | '|')) {
                    target.push(next);
                }
                if target != "/dev/null" && target != "&1" {
                    return None;
                }
            }
            c if UNSAFE.contains(&c) => return None,
            c => {
                word.push(c);
                started = true;
            }
        }
    }
    finish(&mut pieces, &mut word, &mut started);
    Some(pieces)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROJECT: &str = "/Users/me/Movies/RIROTE VISION/reel";

    fn reads(command: &str) -> bool {
        only_reads(command, Path::new(PROJECT))
    }

    #[test]
    fn looking_at_project_files_needs_no_answer() {
        for command in [
            "cat capture/extracted/visible-text.txt",
            "ls",
            "ls -la capture",
            "head -n 80 index.html",
            "tail -20 'capture/log file.txt'",
            "wc -l compositions/*.html",
            "cat \"/Users/me/Movies/RIROTE VISION/reel/index.html\"",
            "cat a.json | head -80",
            "ls capture 2>/dev/null || echo none",
            "pwd; echo; ls",
        ] {
            assert!(reads(command), "{command}");
        }
    }

    #[test]
    fn the_command_from_the_report_needs_an_answer_only_for_python() {
        let base = "cd \"/Users/me/Movies/RIROTE VISION/reel/capture\" && cat extracted/visible-text.txt; echo; cat extracted/fonts-manifest.json | head -80";
        assert!(reads(base));
        assert!(!reads(&format!("{base}; python3 -I -c \"import json\"")));
    }

    #[test]
    fn a_cd_moves_where_relative_paths_are_read_from() {
        assert!(reads("cd capture && cat ../index.html"));
        assert!(!reads("cd capture && cat ../../other/secret.txt"));
        assert!(!reads("cd /Users/me && cat notes.txt"));
        assert!(!reads("cd .. && ls"));
        assert!(!reads("cd"));
    }

    #[test]
    fn files_outside_the_project_need_an_answer() {
        for command in [
            "cat /etc/passwd",
            "cat ../other/a.txt",
            "cat /Users/me/Movies/RIROTE VISION/reel-2/a.txt",
            "head -n 5 /Users/me/.zshrc",
            "ls /",
        ] {
            assert!(!reads(command), "{command}");
        }
    }

    #[test]
    fn anything_that_could_run_or_write_something_else_needs_an_answer() {
        for command in [
            "cat a.txt > b.txt",
            "cat a.txt >> b.txt",
            "cat a.txt 2>err.log",
            "cat $(which secret)",
            "cat `ls`",
            "cat $HOME/a",
            "cat \"$HOME/a\"",
            "cat ~/.ssh/id_rsa",
            "cat {a,/etc/passwd}",
            "cat a\\ b",
            "ls & rm -rf x",
            "ls | sh",
            "ls; rm -rf capture",
            "python3 -c 'print(1)'",
            "head --file=/etc/passwd",
            "tail -f render.log",
            "cat 'unclosed",
            "",
            ";",
        ] {
            assert!(!reads(command), "{command}");
        }
    }
}
