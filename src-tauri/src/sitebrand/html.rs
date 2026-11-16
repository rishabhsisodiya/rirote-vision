//! Just enough of an HTML reader for brand details: the tags and attributes
//! that name the brand, its stylesheets and logo, and a little of its copy.
//! It never runs anything on the page.

use super::address;

const MAX_TEXT: usize = 3000;
const MAX_LINE: usize = 300;

/// What a page says about itself.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Page {
    pub title: Option<String>,
    pub site_name: Option<String>,
    pub description: Option<String>,
    pub theme_colour: Option<String>,
    /// Stylesheet addresses, in page order.
    pub stylesheets: Vec<String>,
    /// Google Fonts stylesheet addresses.
    pub font_links: Vec<String>,
    /// Logo addresses, best first.
    pub logos: Vec<String>,
    /// The contents of `<style>` blocks.
    pub inline_css: Vec<String>,
    /// `style="…"` attributes, each with the tag it was on.
    pub inline_styles: Vec<(String, String)>,
    /// Headings and paragraphs, plain text, for the tone.
    pub copy: Vec<String>,
    /// How many tags the page had, to tell an app shell from a real page.
    pub tag_count: usize,
}

#[derive(Debug)]
struct Tag {
    name: String,
    closing: bool,
    attrs: Vec<(String, String)>,
}

impl Tag {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
    }
}

/// Reads one tag starting just after `<`; returns it and where it ends.
fn read_tag(html: &str, from: usize) -> Option<(Tag, usize)> {
    let bytes = html.as_bytes();
    let mut at = from;
    let closing = bytes.get(at) == Some(&b'/');
    if closing {
        at += 1;
    }
    let name_start = at;
    while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'-' || bytes[at] == b':') {
        at += 1;
    }
    if at == name_start {
        return None;
    }
    let name = html[name_start..at].to_ascii_lowercase();
    let mut attrs = Vec::new();
    loop {
        while at < bytes.len() && (bytes[at].is_ascii_whitespace() || bytes[at] == b'/') {
            at += 1;
        }
        if at >= bytes.len() {
            return None;
        }
        if bytes[at] == b'>' {
            return Some((Tag { name, closing, attrs }, at + 1));
        }
        let key_start = at;
        while at < bytes.len() && !bytes[at].is_ascii_whitespace() && !b"=>/".contains(&bytes[at]) {
            at += 1;
        }
        let key = html[key_start..at].to_ascii_lowercase();
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let mut value = String::new();
        if bytes.get(at) == Some(&b'=') {
            at += 1;
            while at < bytes.len() && bytes[at].is_ascii_whitespace() {
                at += 1;
            }
            match bytes.get(at) {
                Some(&quote) if quote == b'"' || quote == b'\'' => {
                    let end = html[at + 1..].find(quote as char).map_or(html.len(), |end| at + 1 + end);
                    value = decode(&html[at + 1..end]);
                    at = (end + 1).min(html.len());
                }
                _ => {
                    let start = at;
                    while at < bytes.len() && !bytes[at].is_ascii_whitespace() && bytes[at] != b'>' {
                        at += 1;
                    }
                    value = decode(&html[start..at]);
                }
            }
        }
        if key.is_empty() {
            at += 1;
        } else {
            attrs.push((key, value));
        }
    }
}

/// Character references that turn up in names and copy.
pub fn decode(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let Some(semi) = rest.find(';').filter(|&semi| semi <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            "ndash" => Some('–'),
            "mdash" => Some('—'),
            "rsquo" | "lsquo" => Some('\''),
            "rdquo" | "ldquo" => Some('"'),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Text with tags removed and spaces collapsed.
fn plain(fragment: &str) -> String {
    let mut text = String::new();
    let mut inside = false;
    for c in fragment.chars() {
        match c {
            '<' => inside = true,
            '>' if inside => {
                inside = false;
                text.push(' ');
            }
            _ if !inside => text.push(c),
            _ => {}
        }
    }
    decode(&text).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Where the closing tag for `name` starts, at or after `from`.
fn find_close(html: &str, lower: &str, name: &str, from: usize) -> usize {
    lower[from..].find(&format!("</{name}")).map_or(html.len(), |at| from + at)
}

fn says_logo(tag: &Tag) -> bool {
    ["class", "id", "alt", "src", "title"].iter().any(|key| tag.attr(key).is_some_and(|value| value.to_ascii_lowercase().contains("logo")))
}

/// Reads the page at `base` (its final address, for relative links).
pub fn read(html: &str, base: &str) -> Page {
    let lower = html.to_ascii_lowercase();
    let mut page = Page::default();
    let mut logo_imgs = Vec::new();
    let mut icons: Vec<(u32, String)> = Vec::new();
    let mut og_image = None;
    let mut copy_len = 0;
    let mut at = 0;
    let link = |href: &str| address::resolve(base, href);
    while let Some(open) = html[at..].find('<').map(|found| at + found) {
        if lower[open..].starts_with("<!--") {
            at = lower[open..].find("-->").map_or(html.len(), |end| open + end + 3);
            continue;
        }
        let Some((tag, end)) = read_tag(html, open + 1) else {
            at = open + 1;
            continue;
        };
        at = end;
        if tag.closing {
            continue;
        }
        page.tag_count += 1;
        if let Some(style) = tag.attr("style") {
            if !style.trim().is_empty() {
                page.inline_styles.push((tag.name.clone(), style.to_owned()));
            }
        }
        match tag.name.as_str() {
            "script" | "noscript" | "template" | "svg" => {
                let close = find_close(html, &lower, &tag.name, at);
                at = close;
            }
            "style" => {
                let close = find_close(html, &lower, "style", at);
                page.inline_css.push(html[at..close].to_owned());
                at = close;
            }
            "title" => {
                let close = find_close(html, &lower, "title", at);
                let title = plain(&html[at..close]);
                if page.title.is_none() && !title.is_empty() {
                    page.title = Some(title);
                }
                at = close;
            }
            "h1" | "h2" | "h3" | "p" if copy_len < MAX_TEXT => {
                let close = find_close(html, &lower, &tag.name, at);
                let mut text = plain(&html[at..close]);
                if text.chars().count() > MAX_LINE {
                    text = text.chars().take(MAX_LINE).collect::<String>() + "…";
                }
                if text.split_whitespace().count() >= 2 && !page.copy.contains(&text) {
                    copy_len += text.len();
                    page.copy.push(text);
                }
            }
            "meta" => {
                let key = tag.attr("name").or_else(|| tag.attr("property")).unwrap_or("").to_ascii_lowercase();
                let content = tag.attr("content").map(str::trim).filter(|c| !c.is_empty());
                match (key.as_str(), content) {
                    ("theme-color", Some(c)) if page.theme_colour.is_none() => page.theme_colour = Some(c.to_owned()),
                    ("og:site_name" | "application-name", Some(c)) if page.site_name.is_none() => page.site_name = Some(c.to_owned()),
                    ("description" | "og:description", Some(c)) if page.description.is_none() => page.description = Some(c.to_owned()),
                    ("og:image", Some(c)) if og_image.is_none() => og_image = link(c),
                    _ => {}
                }
            }
            "link" => {
                let rel = tag.attr("rel").unwrap_or("").to_ascii_lowercase();
                let Some(href) = tag.attr("href").and_then(link) else { continue };
                if href.contains("fonts.googleapis.com/css") {
                    page.font_links.push(href);
                } else if rel.split_whitespace().any(|r| r == "stylesheet") {
                    page.stylesheets.push(href);
                } else if rel.contains("icon") {
                    let svg = tag.attr("type").is_some_and(|t| t.contains("svg")) || href.to_ascii_lowercase().split('?').next().unwrap_or("").ends_with(".svg");
                    let size = tag.attr("sizes").and_then(|s| s.split('x').next()?.parse::<u32>().ok()).unwrap_or(0);
                    let rank = if svg { 1000 } else if rel.contains("apple-touch-icon") { 180.max(size) } else { size };
                    if !href.to_ascii_lowercase().contains(".ico") {
                        icons.push((rank, href));
                    }
                }
            }
            "img" | "source" if says_logo(&tag) => {
                if let Some(src) = tag.attr("src").or_else(|| tag.attr("data-src")).or_else(|| tag.attr("srcset").and_then(|s| s.split_whitespace().next())).and_then(link) {
                    logo_imgs.push(src);
                }
            }
            _ => {}
        }
    }
    // A logo the page calls a logo first, then the biggest icon, then the share image.
    icons.sort_by(|a, b| b.0.cmp(&a.0));
    let mut logos = logo_imgs;
    logos.extend(icons.into_iter().filter(|(rank, _)| *rank >= 96).map(|(_, href)| href));
    logos.extend(og_image);
    let mut seen = std::collections::HashSet::new();
    logos.retain(|logo| seen.insert(logo.clone()));
    page.logos = logos;
    if let Some(description) = page.description.clone() {
        page.copy.insert(0, description);
    }
    page
}

/// Font families named in a Google Fonts address:
/// `…/css2?family=Playfair+Display:wght@700&family=Inter` → both.
pub fn google_families(url: &str) -> Vec<String> {
    let query = url.split('?').nth(1).unwrap_or("");
    query
        .split('&')
        .filter_map(|pair| pair.strip_prefix("family="))
        .flat_map(|value| value.split('|'))
        .map(|family| family.split(':').next().unwrap_or("").replace(['+'], " ").replace("%20", " ").trim().to_owned())
        .filter(|family| !family.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r##"<!doctype html><html><head>
<title>Home &amp; Shop | Angaraa Charcoal</title>
<meta name="theme-color" content="#1A1A1A">
<meta property="og:site_name" content="ANGARAA">
<meta name="description" content="Clean-burning coconut shell charcoal, made in Rajasthan.">
<meta property="og:image" content="/share.jpg">
<link rel="stylesheet" href="/css/site.css?v=3">
<link rel="preconnect" href="https://fonts.gstatic.com">
<link href="https://fonts.googleapis.com/css2?family=Playfair+Display:wght@700&family=Inter:wght@400;600&display=swap" rel="stylesheet">
<link rel="icon" href="/favicon.ico">
<link rel="apple-touch-icon" href="/touch.png">
<link rel="icon" type="image/svg+xml" href="/mark.svg">
<style>:root{--brand:#E4572E}</style>
<script>var x = "<p>not copy</p>";</script>
</head><body style="background:#FAF7F2">
<!-- <img class="logo" src="/old.png"> -->
<header><a href="/"><img class=site-logo src="img/logo.svg" alt="Angaraa"></a></header>
<h1>Fire that <em>lasts</em></h1>
<p>Our charcoal burns hot and clean for hours.</p>
<p>Hi</p>
</body></html>"##;

    #[test]
    fn a_page_gives_its_brand_details() {
        let page = read(PAGE, "https://theangaraa.com/");
        assert_eq!(page.title.as_deref(), Some("Home & Shop | Angaraa Charcoal"));
        assert_eq!(page.site_name.as_deref(), Some("ANGARAA"));
        assert_eq!(page.theme_colour.as_deref(), Some("#1A1A1A"));
        assert_eq!(page.stylesheets, vec!["https://theangaraa.com/css/site.css?v=3"]);
        assert_eq!(page.font_links.len(), 1);
        assert_eq!(
            page.logos,
            vec!["https://theangaraa.com/img/logo.svg", "https://theangaraa.com/mark.svg", "https://theangaraa.com/touch.png", "https://theangaraa.com/share.jpg"]
        );
        assert_eq!(page.inline_css, vec![":root{--brand:#E4572E}"]);
        assert_eq!(page.inline_styles, vec![("body".to_owned(), "background:#FAF7F2".to_owned())]);
        assert_eq!(page.copy, vec!["Clean-burning coconut shell charcoal, made in Rajasthan.", "Fire that lasts", "Our charcoal burns hot and clean for hours."]);
        assert!(!page.copy.iter().any(|line| line.contains("not copy")));
    }

    #[test]
    fn google_font_links_name_their_families() {
        assert_eq!(
            google_families("https://fonts.googleapis.com/css2?family=Playfair+Display:wght@700&family=Inter:wght@400;600&display=swap"),
            vec!["Playfair Display", "Inter"]
        );
        assert_eq!(google_families("https://fonts.googleapis.com/css?family=Open+Sans:400,700|Lora"), vec!["Open Sans", "Lora"]);
    }

    #[test]
    fn broken_markup_does_not_stop_the_reader() {
        let page = read("<html><title>Only <b>this</b></title><p unclosed='x <img", "https://a.com/");
        assert_eq!(page.title.as_deref(), Some("Only this"));
        assert_eq!(decode("A &#38; B &#x26; C &bogus; D &"), "A & B & C &bogus; D &");
    }
}
