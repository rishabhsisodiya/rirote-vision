//! Downloads with curl (it ships with macOS): http(s) only, a byte cap and a
//! time limit on every file, and redirects that stay on http(s).

use std::path::Path;
use std::time::Duration;

use crate::error::{AppError, AppResult};
use crate::reference::run::{self, Cancel};
use crate::shell_env;

/// Sites that turn away unknown clients usually let a browser in.
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";

pub struct Fetched {
    pub status: u16,
    pub content_type: String,
    /// Where it ended up after redirects.
    pub final_url: String,
    pub body: Vec<u8>,
}

/// Downloads `url` into `out` and reads it back, cut at `max_bytes`.
pub async fn get(url: &str, out: &Path, max_bytes: u64, limit: Duration, cancel: &Cancel) -> AppResult<Fetched> {
    let curl = shell_env::require_program("curl", "curl comes with macOS; check the PATH.")?;
    let mut command = shell_env::command(&curl);
    command
        .args(["-sS", "-L", "--max-redirs", "5", "--proto", "=http,https", "--proto-redir", "=http,https", "--compressed"])
        .args(["--max-filesize", &max_bytes.to_string(), "--max-time", &limit.as_secs().to_string()])
        .args(["-A", USER_AGENT, "-H", "Accept-Language: en-IN,en;q=0.9", "-w", "%{http_code}\t%{content_type}\t%{url_effective}", "-o"])
        .arg(out)
        .arg(url);
    let output = run::run(command, limit + Duration::from_secs(5), "reading the website", cancel).await?;
    if !output.status.success() {
        let _ = std::fs::remove_file(out);
        return Err(AppError::Refused(curl_reason(output.status.code()).into()));
    }
    let written = String::from_utf8_lossy(&output.stdout);
    let mut parts = written.splitn(3, '\t');
    let status = parts.next().and_then(|code| code.trim().parse().ok()).unwrap_or(0);
    let content_type = parts.next().unwrap_or("").trim().to_ascii_lowercase();
    let final_url = parts.next().map(str::trim).filter(|u| !u.is_empty()).unwrap_or(url).to_owned();
    let mut body = std::fs::read(out).unwrap_or_default();
    body.truncate(usize::try_from(max_bytes).unwrap_or(usize::MAX));
    Ok(Fetched { status, content_type, final_url, body })
}

fn curl_reason(code: Option<i32>) -> &'static str {
    match code {
        Some(6) => "That website could not be found. Check the address.",
        Some(7) => "The website did not answer. Check the internet connection, or try again later.",
        Some(28) => "The website took too long to answer.",
        Some(35 | 51 | 53 | 54 | 58 | 59 | 60 | 77 | 83) => "The website's security certificate could not be checked, so it was not read.",
        Some(47) => "The website redirected too many times.",
        Some(63) => "The file was too large to read.",
        _ => "The website could not be read. Check the address and the internet connection.",
    }
}

/// The image type of a downloaded logo, judged by its contents. SVGs that
/// could run anything (scripts, event handlers, embedded HTML) are refused.
pub fn logo_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg");
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let lower = text.trim_start_matches('\u{feff}').trim_start().to_ascii_lowercase();
    let starts_right = ["<svg", "<?xml", "<!--", "<!doctype svg"].iter().any(|start| lower.starts_with(start));
    let unsafe_parts = ["<script", "javascript:", "<foreignobject", "<!entity", "<iframe", "<embed", "<object"];
    (starts_right && lower.contains("<svg") && !unsafe_parts.iter().any(|part| lower.contains(part)) && !has_event_handler(&lower)).then_some("svg")
}

/// `onload=`, `onclick =` and the like.
fn has_event_handler(lower: &str) -> bool {
    let bytes = lower.as_bytes();
    lower.match_indices("on").any(|(at, _)| {
        let before = at.checked_sub(1).map(|b| bytes[b]);
        if !matches!(before, Some(b' ' | b'\n' | b'\r' | b'\t' | b'/' | b'"' | b'\'')) {
            return false;
        }
        let mut end = at + 2;
        while end < bytes.len() && bytes[end].is_ascii_alphabetic() {
            end += 1;
        }
        while end < bytes.len() && bytes[end].is_ascii_whitespace() {
            end += 1;
        }
        end > at + 2 && bytes.get(end) == Some(&b'=')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logos_are_judged_by_their_contents() {
        assert_eq!(logo_type(b"\x89PNG\r\n\x1a\nxxxx"), Some("png"));
        assert_eq!(logo_type(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(logo_type(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(logo_type(b"<?xml version=\"1.0\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\"><path d=\"M0 0\"/></svg>"), Some("svg"));
        assert_eq!(logo_type(b"\xEF\xBB\xBF  <svg viewBox=\"0 0 1 1\" font-family=\"Montserrat\"></svg>"), Some("svg"), "a BOM and a word containing `on` are fine");
        assert_eq!(logo_type(b"<html><body>Not found</body></html>"), None);
        assert_eq!(logo_type(b"<svg onload=\"alert(1)\"></svg>"), None);
        assert_eq!(logo_type(b"<svg><a href=\"javascript:x()\"/></svg>"), None);
        assert_eq!(logo_type(b"<svg><script>x()</script></svg>"), None);
        assert_eq!(logo_type(b"<svg><foreignObject><div/></foreignObject></svg>"), None);
        assert_eq!(logo_type(b"GIF89a"), None);
    }
}
