//! Reading a brand kit off a website: its name, colours, fonts, logo and,
//! with Claude Code, its tone. The result is only a suggestion; nothing is
//! saved until the user applies it to a kit.
//!
//! The app reads the page and its stylesheets itself; Claude only chooses
//! among what was found, with no tools, so text on a page cannot make it act.

mod address;
mod css;
mod fetch;
mod html;
mod pick;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;

use crate::brandkit::Colour;
use crate::claude_once;
use crate::error::{AppError, AppResult};
use crate::reference::run::{stopped, Cancel};
use pick::{Candidates, Choice};

const PAGE_BYTES: u64 = 3 * 1024 * 1024;
const PAGE_LIMIT: Duration = Duration::from_secs(20);
const STYLESHEETS: usize = 5;
const STYLESHEET_BYTES: u64 = 600 * 1024;
const STYLESHEET_LIMIT: Duration = Duration::from_secs(10);
const LOGO_TRIES: usize = 4;
const LOGO_BYTES: u64 = 5 * 1024 * 1024;
const LOGO_LIMIT: Duration = Duration::from_secs(15);
const CLAUDE_LIMIT: Duration = Duration::from_secs(150);
/// Inside `brand-kits/` so the window may show the logo; the name cannot be a kit id.
const SCRATCH: &str = "_site-reads";

/// What was found on a site, for the user to review.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Findings {
    /// Where the page ended up after redirects.
    pub url: String,
    pub name: Option<String>,
    pub colours: Vec<Colour>,
    pub heading_font: Option<String>,
    pub body_font: Option<String>,
    pub tone: Option<String>,
    /// A checked copy of the logo, to add with the kit's logo command.
    pub logo_file: Option<String>,
    /// Whether Claude Code chose the roles and wrote the tone.
    pub ai_used: bool,
    /// Plain-language remarks on what could not be read.
    pub notes: Vec<String>,
}

/// One read at a time; starting another stops the one before.
pub struct SiteReader {
    root: PathBuf,
    current: Mutex<(u64, Cancel)>,
}

impl SiteReader {
    pub fn new(data_dir: &Path) -> Self {
        Self { root: data_dir.join("brand-kits").join(SCRATCH), current: Mutex::new((0, Cancel::default())) }
    }

    pub fn cancel(&self) {
        self.current.lock().unwrap_or_else(|p| p.into_inner()).1.raise();
    }

    /// Reads `website` and suggests a kit. `model` is the assistant's model.
    pub async fn read(&self, website: &str, model: &str) -> AppResult<Findings> {
        let url = address::normalise(website)?;
        let (generation, cancel) = {
            let mut current = self.current.lock().unwrap_or_else(|p| p.into_inner());
            current.1.raise();
            current.0 += 1;
            current.1 = Cancel::default();
            (current.0, current.1.clone())
        };
        // Earlier reads' files go; this read's logo stays until the next one.
        let dir = self.root.join(format!("read-{generation}"));
        if let Ok(entries) = std::fs::read_dir(&self.root) {
            for entry in entries.filter_map(Result::ok) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
        std::fs::create_dir_all(&dir)?;
        let findings = read_site(&url, &dir, model, &cancel).await;
        if cancel.raised() {
            return Err(stopped());
        }
        findings
    }
}

async fn read_site(url: &str, dir: &Path, model: &str, cancel: &Cancel) -> AppResult<Findings> {
    let page = fetch::get(url, &dir.join("page.html"), PAGE_BYTES, PAGE_LIMIT, cancel).await?;
    if page.status >= 400 || page.status == 0 {
        return Err(AppError::Refused(format!("The website answered with an error ({}). Check the address.", page.status)));
    }
    if !page.content_type.is_empty() && !page.content_type.contains("html") {
        return Err(AppError::InvalidInput("That address is a file, not a web page. Use the site's home page.".into()));
    }
    address::check_public(&page.final_url)
        .map_err(|_| AppError::InvalidInput("The website sent the app to a private address, so it was not read.".into()))?;
    let base = page.final_url;
    let markup = html::read(&String::from_utf8_lossy(&page.body), &base);
    let _ = std::fs::remove_file(dir.join("page.html"));

    let mut declarations = Vec::new();
    for (tag, style) in &markup.inline_styles {
        declarations.extend(css::inline(tag, style));
    }
    for block in &markup.inline_css {
        declarations.extend(css::declarations(block));
    }
    let mut unread_stylesheets = 0;
    for (at, sheet) in markup.stylesheets.iter().take(STYLESHEETS).enumerate() {
        cancel.check()?;
        if address::check_public(sheet).is_err() {
            continue;
        }
        match fetch::get(sheet, &dir.join(format!("style-{at}.css")), STYLESHEET_BYTES, STYLESHEET_LIMIT, cancel).await {
            Ok(file) if file.status < 400 => declarations.extend(css::declarations(&String::from_utf8_lossy(&file.body))),
            Err(error) if cancel.raised() => return Err(error),
            _ => unread_stylesheets += 1,
        }
        let _ = std::fs::remove_file(dir.join(format!("style-{at}.css")));
    }
    let google: Vec<String> = markup.font_links.iter().flat_map(|link| html::google_families(link)).collect();

    let host = address::host(&base).unwrap_or_default();
    let candidates = Candidates {
        url: base.clone(),
        names: pick::names(markup.site_name.as_deref(), markup.title.as_deref(), &host),
        colours: css::rank_colours(&declarations, markup.theme_colour.as_deref()).into_iter().take(pick::MAX_COLOUR_CANDIDATES).collect(),
        fonts: css::rank_fonts(&declarations, &google).into_iter().take(pick::MAX_FONT_CANDIDATES).collect(),
        copy: markup.copy.clone(),
    };

    let mut notes = Vec::new();
    if markup.copy.is_empty() && markup.tag_count < 40 {
        notes.push("This site builds its page with JavaScript, so only a little could be read. Check each value.".to_owned());
    }
    if unread_stylesheets > 0 {
        notes.push(format!("{unread_stylesheets} of the site's stylesheets could not be read."));
    }
    if candidates.colours.is_empty() {
        notes.push("No colours were found on the page.".to_owned());
    }
    if candidates.fonts.is_empty() {
        notes.push("No web fonts were found; the site may use the Mac's own fonts.".to_owned());
    }

    let logo_file = download_logo(&markup.logos, dir, cancel).await?;
    if logo_file.is_none() && !markup.logos.is_empty() {
        notes.push("The site's logo could not be used (it has to be an SVG, PNG, JPEG or WebP).".to_owned());
    }

    let (mut choice, ai_used) = match choose(&candidates, dir, model, cancel).await {
        Ok(choice) => (choice, true),
        Err(_) if cancel.raised() => return Err(stopped()),
        Err(error) => {
            notes.push(format!("Claude Code was not used ({}), so the colour roles are a guess and the tone is left for you.", crate::shell_env::first_line(&error.to_string()).trim_end_matches('.')));
            (pick::fallback(&candidates), false)
        }
    };
    // The name the site gives itself beats any reading of its title.
    if let Some((name, pick::NAME_TAG)) = candidates.names.first() {
        choice.name = Some(name.clone());
    }
    Ok(Findings {
        url: base,
        name: choice.name,
        colours: choice.colours,
        heading_font: choice.heading_font,
        body_font: choice.body_font,
        tone: choice.tone,
        logo_file: logo_file.map(|file| file.to_string_lossy().into_owned()),
        ai_used,
        notes,
    })
}

/// The first logo address that downloads as a safe image, saved as `logo.<type>`.
async fn download_logo(logos: &[String], dir: &Path, cancel: &Cancel) -> AppResult<Option<PathBuf>> {
    let draft = dir.join("logo.download");
    for logo in logos.iter().take(LOGO_TRIES) {
        cancel.check()?;
        if address::check_public(logo).is_err() {
            continue;
        }
        let Ok(file) = fetch::get(logo, &draft, LOGO_BYTES, LOGO_LIMIT, cancel).await else {
            if cancel.raised() {
                return Err(stopped());
            }
            continue;
        };
        let _ = std::fs::remove_file(&draft);
        if file.status >= 400 || file.body.len() as u64 >= LOGO_BYTES {
            continue;
        }
        if let Some(kind) = fetch::logo_type(&file.body) {
            let target = dir.join(format!("logo.{kind}"));
            std::fs::write(&target, &file.body)?;
            return Ok(Some(target));
        }
    }
    Ok(None)
}

/// Claude chooses among the candidates; a wrong-shaped answer gets one retry.
async fn choose(candidates: &Candidates, dir: &Path, model: &str, cancel: &Cancel) -> AppResult<Choice> {
    if candidates.colours.is_empty() && candidates.fonts.is_empty() && candidates.copy.is_empty() {
        return Err(AppError::InvalidInput("too little was found to choose from".into()));
    }
    let question = pick::prompt(candidates);
    let ask = |prompt: &str| {
        let prompt = prompt.to_owned();
        async move {
            let ask = claude_once::Ask { prompt: &prompt, system_prompt: pick::SYSTEM_PROMPT, tools: &[], dir, model, limit: CLAUDE_LIMIT, what: "reading the brand" };
            claude_once::ask(ask, cancel).await.map(|reply| reply.text)
        }
    };
    let reply = ask(&question).await?;
    match pick::parse(&reply, candidates) {
        Ok(choice) => Ok(choice),
        Err(reason) => {
            let retry = format!("{question}\n\nYour previous answer:\n{reply}\n\nIt was not valid: {reason}\nReturn the corrected, complete JSON object only.");
            let reply = ask(&retry).await?;
            pick::parse(&reply, candidates).map_err(|reason| AppError::ToolFailed(format!("its answer came back in the wrong shape twice: {reason}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "reads real websites and runs Claude Code on the signed-in plan"]
    async fn a_real_site_becomes_a_kit_suggestion() {
        let data = tempfile::tempdir().unwrap();
        let reader = SiteReader::new(data.path());
        for site in ["theangaraa.com", "riroteglobal.com"] {
            let found = reader.read(site, "haiku").await.expect("the site is read");
            println!("{site}: {found:#?}");
            assert!(found.name.is_some(), "{site} has a name");
            assert!(!found.colours.is_empty(), "{site} has colours");
            assert!(found.ai_used, "{site}: Claude chose the roles");
            if let Some(logo) = &found.logo_file {
                assert!(Path::new(logo).is_file());
            }
        }
        assert_eq!(reader.read("localhost:3000", "").await.unwrap_err().code(), "invalid_input");
    }

    #[tokio::test]
    #[ignore = "reads a real website"]
    async fn starting_a_read_stops_the_one_before() {
        let data = tempfile::tempdir().unwrap();
        let reader = std::sync::Arc::new(SiteReader::new(data.path()));
        let first = tokio::spawn({
            let reader = reader.clone();
            async move { reader.read("theangaraa.com", "haiku").await }
        });
        tokio::time::sleep(Duration::from_millis(300)).await;
        reader.cancel();
        assert_eq!(first.await.unwrap().unwrap_err().to_string(), "Stopped.");
    }
}
