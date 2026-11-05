//! yt-dlp: downloads a reference from a link, and searches for references.
//! The official macOS build is fetched from its GitHub releases and checked
//! against the checksums published with it.

use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use super::model::Found;
use super::run::{self, Cancel};
use crate::error::{AppError, AppResult};
use crate::shell_env;

const RELEASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";
const ASSET: &str = "yt-dlp_macos";
const FETCH_LIMIT: Duration = Duration::from_secs(300);
const INFO_LIMIT: Duration = Duration::from_secs(60);
const DOWNLOAD_LIMIT: Duration = Duration::from_secs(20 * 60);
const SEARCH_LIMIT: Duration = Duration::from_secs(90);
/// Larger downloads are refused: a reference is studied, not archived.
pub const MAX_BYTES: &str = "500M";

pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("tools").join("yt-dlp")
}

pub fn is_ready(data_dir: &Path) -> bool {
    path(data_dir).is_file()
}

/// Its version, which is its release date.
pub async fn version(data_dir: &Path) -> Option<String> {
    let mut command = shell_env::command(&path(data_dir));
    command.arg("--version");
    let output = shell_env::run(command, Duration::from_secs(30), "yt-dlp").await.ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Downloads the latest release, checks it and puts it in place. Also the update.
pub async fn install(data_dir: &Path) -> AppResult<()> {
    let curl = shell_env::require_program("curl", "curl comes with macOS; check the PATH.")?;
    let tools = data_dir.join("tools");
    std::fs::create_dir_all(&tools)?;
    let draft = tools.join("yt-dlp.download");
    let sums = tools.join("yt-dlp.sums");
    for (url, file) in [(format!("{RELEASE}/{ASSET}"), &draft), (format!("{RELEASE}/SHA2-256SUMS"), &sums)] {
        let mut command = shell_env::command(&curl);
        command.args(["-fsSL", "--retry", "2", "-o"]).arg(file).arg(&url);
        let output = shell_env::run(command, FETCH_LIMIT, "the yt-dlp download").await?;
        if !output.status.success() {
            let _ = std::fs::remove_file(&draft);
            return Err(AppError::ToolFailed(format!(
                "yt-dlp could not be downloaded. Check the internet connection.\n{}",
                shell_env::failure_text(&output)
            )));
        }
    }
    let published = std::fs::read_to_string(&sums)?;
    let _ = std::fs::remove_file(&sums);
    let expected = expected_sum(&published, ASSET)
        .ok_or_else(|| AppError::ToolFailed("yt-dlp's checksum list does not name the macOS build.".into()))?;
    let actual = sha256(&draft).await?;
    if actual != expected {
        let _ = std::fs::remove_file(&draft);
        return Err(AppError::ToolFailed(
            "The downloaded yt-dlp did not match its published checksum, so it was thrown away.".into(),
        ));
    }
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&draft, std::fs::Permissions::from_mode(0o755))?;
    std::fs::rename(&draft, path(data_dir))?;
    Ok(())
}

fn expected_sum(list: &str, asset: &str) -> Option<String> {
    list.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let sum = parts.next()?;
        (parts.next()? == asset).then(|| sum.to_ascii_lowercase())
    })
}

pub(crate) async fn sha256(file: &Path) -> AppResult<String> {
    let shasum = shell_env::require_program("shasum", "shasum comes with macOS; check the PATH.")?;
    let mut command = shell_env::command(&shasum);
    command.args(["-a", "256"]).arg(file);
    let output = shell_env::run(command, Duration::from_secs(60), "shasum").await?;
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| AppError::ToolFailed("The download could not be checked.".into()))
}

/// Refuses anything but a public web link: no files, no local network.
pub fn check_link(link: &str) -> AppResult<String> {
    let link = link.trim();
    let refuse = || AppError::InvalidInput("Paste the full link of one video, starting with https://".into());
    let rest = link.strip_prefix("https://").or_else(|| link.strip_prefix("http://")).ok_or_else(refuse)?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit('@').next().unwrap_or(host);
    let name = host.split(':').next().unwrap_or(host).trim_matches(['[', ']']).to_ascii_lowercase();
    if name.is_empty() || !name.contains('.') && name.parse::<IpAddr>().is_err() {
        return Err(refuse());
    }
    let private = match name.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_unspecified(),
        Ok(IpAddr::V6(ip)) => ip.is_loopback() || ip.is_unspecified() || (ip.segments()[0] & 0xfe00) == 0xfc00,
        Err(_) => name == "localhost" || name.ends_with(".localhost") || name.ends_with(".local"),
    };
    if private {
        return Err(AppError::InvalidInput("Links to this Mac or the local network cannot be studied.".into()));
    }
    Ok(link.to_owned())
}

/// The platform a link belongs to, for the library and its id.
pub fn platform_of(extractor: &str) -> String {
    let lower = extractor.to_ascii_lowercase();
    for (needle, name) in [("youtube", "youtube"), ("instagram", "instagram"), ("tiktok", "tiktok"), ("twitter", "x")] {
        if lower.contains(needle) {
            return name.into();
        }
    }
    "web".into()
}

fn base(data_dir: &Path) -> AppResult<tokio::process::Command> {
    if !is_ready(data_dir) {
        return Err(AppError::ToolMissing("Reference downloads are not set up. Install yt-dlp from Check Setup.".into()));
    }
    let mut command = shell_env::command(&path(data_dir));
    // A config file of the user's own must not change what the app runs.
    command.args(["--ignore-config", "--no-warnings", "--no-playlist"]);
    Ok(command)
}

/// What a link points at, without downloading it.
pub struct LinkInfo {
    pub id: String,
    pub platform: String,
    pub title: Option<String>,
    pub creator: Option<String>,
    pub duration: Option<f64>,
}

pub async fn info(data_dir: &Path, link: &str, cancel: &Cancel) -> AppResult<LinkInfo> {
    let mut command = base(data_dir)?;
    command.args(["-J", "--skip-download", "--", link]);
    let output = run::run(command, INFO_LIMIT, "yt-dlp", cancel).await?;
    if !output.status.success() {
        return Err(explain(&shell_env::failure_text(&output)));
    }
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| AppError::ToolFailed("yt-dlp gave an answer the app could not read.".into()))?;
    if value.get("_type").and_then(Value::as_str) == Some("playlist") {
        return Err(AppError::InvalidInput("That link is a playlist or a profile. Paste the link of one video.".into()));
    }
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    Ok(LinkInfo {
        id: text("id").ok_or_else(|| AppError::ToolFailed("yt-dlp did not say what the video is.".into()))?,
        platform: platform_of(&text("extractor_key").unwrap_or_default()),
        title: text("title"),
        creator: text("channel").or_else(|| text("uploader")),
        duration: value.get("duration").and_then(Value::as_f64),
    })
}

/// Downloads the video as `<dir>/source.mp4`.
pub async fn download(data_dir: &Path, link: &str, dir: &Path, max_minutes: u32, cancel: &Cancel) -> AppResult<PathBuf> {
    let mut command = base(data_dir)?;
    if let Some(ffmpeg) = shell_env::find_program("ffmpeg") {
        command.arg("--ffmpeg-location").arg(ffmpeg);
    }
    command
        .args(["-f", "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/bv*+ba/b", "--merge-output-format", "mp4"])
        .args(["--max-filesize", MAX_BYTES])
        .args(["--match-filter", &format!("duration <=? {}", max_minutes * 60)])
        .arg("-o")
        .arg(dir.join("source.%(ext)s"))
        .args(["--", link]);
    let output = run::run(command, DOWNLOAD_LIMIT, "yt-dlp", cancel).await?;
    let said = String::from_utf8_lossy(&output.stdout).into_owned();
    if said.contains("does not pass filter") {
        return Err(AppError::InvalidInput(format!("That video is longer than {max_minutes} minutes. Pick a shorter one, or raise the limit in Settings.")));
    }
    if !output.status.success() {
        return Err(explain(&shell_env::failure_text(&output)));
    }
    find_source(dir).ok_or_else(|| {
        if said.contains("larger than max-filesize") || said.contains("File is larger") {
            AppError::InvalidInput("That video is larger than 500 MB.".into())
        } else {
            AppError::ToolFailed("The download finished but no video was written.".into())
        }
    })
}

pub fn find_source(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.filter_map(Result::ok).map(|entry| entry.path()).find(|path| {
        path.file_stem().is_some_and(|stem| stem == "source")
            && path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| {
                ["mp4", "mov", "m4v", "webm", "mkv"].contains(&ext.to_ascii_lowercase().as_str())
            })
    })
}

/// yt-dlp's reasons, in words a person can act on.
fn explain(text: &str) -> AppError {
    let lower = text.to_ascii_lowercase();
    let message = if lower.contains("private") {
        "That video is private."
    } else if lower.contains("login") || lower.contains("sign in") || lower.contains("cookies") {
        "That site wants you signed in to see this video. Download it yourself and drop the file in instead."
    } else if lower.contains("not available") || lower.contains("removed") || lower.contains("404") {
        "That video is not available any more, or not in this country."
    } else if lower.contains("unsupported url") {
        "That site is not one yt-dlp can download from. Download the video yourself and drop the file in."
    } else if lower.contains("unable to download") || lower.contains("connection") || lower.contains("resolve") {
        "The video could not be reached. Check the internet connection."
    } else {
        return AppError::ToolFailed(format!("yt-dlp could not get that video.\n{}", text.trim()));
    };
    AppError::Refused(message.into())
}

/// YouTube search, or an Instagram hashtag with the browser's sign-in.
pub async fn search(
    data_dir: &Path,
    query: &str,
    source: &str,
    cookie_browser: Option<&str>,
    limit: usize,
    cancel: &Cancel,
) -> AppResult<Vec<Found>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let target = match source {
        "youtube" => format!("ytsearch{limit}:{query}"),
        "instagram" => {
            let tag: String = query.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect();
            if tag.is_empty() {
                return Err(AppError::InvalidInput("Instagram search takes one hashtag, like motiongraphics.".into()));
            }
            format!("https://www.instagram.com/explore/tags/{tag}/")
        }
        _ => return Err(AppError::InvalidInput("Search YouTube or Instagram.".into())),
    };
    let mut command = base(data_dir)?;
    if let Some(browser) = cookie_browser {
        command.args(["--cookies-from-browser", browser]);
    }
    command.args(["--flat-playlist", "-J", "--playlist-end", &limit.to_string(), "--", &target]);
    let output = run::run(command, SEARCH_LIMIT, "yt-dlp search", cancel).await?;
    if !output.status.success() {
        let said = shell_env::failure_text(&output);
        if source == "instagram" {
            return Err(AppError::Refused(
                "Instagram did not answer the search. Sign in to Instagram in the browser chosen in Settings, then try again. Pasting a reel's link always works.".into(),
            ));
        }
        return Err(explain(&said));
    }
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| AppError::ToolFailed("yt-dlp gave an answer the app could not read.".into()))?;
    Ok(found(&value, source))
}

fn found(value: &Value, source: &str) -> Vec<Found> {
    let entries = value.get("entries").and_then(Value::as_array).cloned().unwrap_or_default();
    entries
        .iter()
        .filter_map(|entry| {
            let text = |key: &str| entry.get(key).and_then(Value::as_str).map(str::to_owned);
            let id = text("id")?;
            let url = text("url").or_else(|| text("webpage_url"))?;
            if entry.get("live_status").and_then(Value::as_str).is_some_and(|live| live == "is_live") {
                return None;
            }
            Some(Found {
                id,
                url,
                title: text("title").unwrap_or_else(|| "Untitled".into()),
                creator: text("channel").or_else(|| text("uploader")),
                duration: entry.get("duration").and_then(Value::as_f64),
                views: entry.get("view_count").and_then(Value::as_u64),
                platform: source.into(),
                thumbnail: entry
                    .get("thumbnails")
                    .and_then(Value::as_array)
                    .and_then(|list| list.iter().filter_map(|t| t.get("url").and_then(Value::as_str)).next().map(str::to_owned))
                    .or_else(|| text("thumbnail")),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_must_be_public_web_addresses() {
        assert!(check_link(" https://www.youtube.com/shorts/abc ").is_ok());
        assert!(check_link("https://instagram.com/reel/xyz/?igsh=1").is_ok());
        for bad in [
            "file:///etc/passwd",
            "youtube.com/watch?v=1",
            "https://localhost:8080/a",
            "http://127.0.0.1/a",
            "http://192.168.1.4/v.mp4",
            "http://[::1]/v",
            "https://printer.local/x",
            "https://user@10.0.0.2/x",
            "https://intranet/x",
        ] {
            assert!(check_link(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_published_sum_is_found_for_the_macos_build_only() {
        let list = "aaa  yt-dlp\nBBB  yt-dlp_macos\nccc  yt-dlp_macos.zip\n";
        assert_eq!(expected_sum(list, "yt-dlp_macos").as_deref(), Some("bbb"));
        assert_eq!(expected_sum(list, "nothing"), None);
    }

    #[test]
    fn search_results_skip_live_streams_and_keep_the_first_thumbnail() {
        let value: Value = serde_json::json!({"entries": [
            {"id": "a", "url": "https://youtu.be/a", "title": "One", "channel": "Chan", "duration": 31.0,
             "view_count": 1200, "thumbnails": [{"url": "https://i.ytimg.com/a.jpg"}]},
            {"id": "b", "url": "https://youtu.be/b", "title": "Live", "live_status": "is_live"},
            {"title": "no id"}
        ]});
        let list = found(&value, "youtube");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].thumbnail.as_deref(), Some("https://i.ytimg.com/a.jpg"));
        assert_eq!(list[0].views, Some(1200));
    }

    #[test]
    fn platforms_come_from_the_extractor() {
        assert_eq!(platform_of("Youtube"), "youtube");
        assert_eq!(platform_of("InstagramIOS"), "instagram");
        assert_eq!(platform_of("Twitter"), "x");
        assert_eq!(platform_of("Vimeo"), "web");
    }
}
