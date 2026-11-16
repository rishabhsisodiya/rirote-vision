//! Colours and fonts a site's stylesheets use, ranked by how much they
//! matter: what the page background, text, buttons and headings use counts
//! for more than a border.

use std::collections::HashMap;

/// One `property: value` with the selector it sits under.
#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    pub selector: String,
    pub property: String,
    pub value: String,
}

/// Every declaration in a stylesheet, in order. Nested at-rules (`@media`)
/// are read through; `@keyframes` are left out, and `@font-face` blocks keep
/// the selector `@font-face` (they name the web fonts the site loads).
pub fn declarations(css: &str) -> Vec<Declaration> {
    let css = strip_comments(css);
    let mut out = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut boundary = 0;
    for (at, c) in css.char_indices() {
        match c {
            '{' => {
                let head = &css[boundary..at];
                // `@import …;` and stray declarations before a rule are not its selector.
                let selector = head.rsplit(';').next().unwrap_or("").trim().to_owned();
                stack.push(selector);
                boundary = at + 1;
            }
            '}' => {
                if let Some(selector) = stack.last() {
                    let skipped = stack.iter().any(|s| s.contains("keyframes"));
                    if selector.starts_with("@font-face") {
                        out.extend(block("@font-face", &css[boundary..at]).into_iter().filter(|d| d.property == "font-family"));
                    } else if !selector.starts_with('@') && !skipped {
                        out.extend(block(selector, &css[boundary..at]));
                    }
                }
                stack.pop();
                boundary = at + 1;
            }
            _ => {}
        }
    }
    out
}

/// The declarations in a `style="…"` attribute on a `tag`.
pub fn inline(tag: &str, style: &str) -> Vec<Declaration> {
    block(tag, style)
}

fn block(selector: &str, body: &str) -> Vec<Declaration> {
    body.split(';')
        .filter_map(|part| {
            let (property, value) = part.split_once(':')?;
            let property = property.trim().to_ascii_lowercase();
            let value = value.trim().trim_end_matches("!important").trim().to_owned();
            (!property.is_empty() && !value.is_empty()).then(|| Declaration { selector: selector.to_owned(), property, value })
        })
        .collect()
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        rest = rest[start + 2..].find("*/").map_or("", |end| &rest[start + 2 + end + 2..]);
    }
    out.push_str(rest);
    out
}

/// `var(--x)` replaced by the custom property's value, a few levels deep.
fn substitute(value: &str, vars: &HashMap<String, String>) -> String {
    let mut value = value.to_owned();
    for _ in 0..4 {
        let Some(start) = value.find("var(") else { break };
        let Some(len) = matching_paren(&value[start + 3..]) else { break };
        let inner = &value[start + 4..start + 3 + len];
        let (name, fallback) = inner.split_once(',').map_or((inner.trim(), None), |(n, f)| (n.trim(), Some(f.trim())));
        let replacement = vars.get(name).cloned().or_else(|| fallback.map(str::to_owned)).unwrap_or_default();
        value = format!("{}{}{}", &value[..start], replacement, &value[start + 4 + len..]);
    }
    value
}

/// Length to the `)` matching the `(` at the start of `text`.
fn matching_paren(text: &str) -> Option<usize> {
    let mut depth = 0;
    for (at, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(at);
                }
            }
            _ => {}
        }
    }
    None
}

// --- colours -----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.0, self.1, self.2)
    }

    /// Saturation and lightness, 0–1 (HSL).
    pub fn saturation_lightness(self) -> (f64, f64) {
        let [r, g, b] = [self.0, self.1, self.2].map(|v| f64::from(v) / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let lightness = (max + min) / 2.0;
        let saturation = if max == min { 0.0 } else { (max - min) / (1.0 - (2.0 * lightness - 1.0).abs()) };
        (saturation, lightness)
    }

    fn distance(self, other: Self) -> f64 {
        let d = |a: u8, b: u8| (f64::from(a) - f64::from(b)).powi(2);
        (d(self.0, other.0) + d(self.1, other.1) + d(self.2, other.2)).sqrt()
    }
}

pub fn parse_hex(text: &str) -> Option<Rgb> {
    let digits = text.trim().strip_prefix('#')?;
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let pair = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
    let single = |at: usize| u8::from_str_radix(&digits[at..=at], 16).ok().map(|v| v * 17);
    match digits.len() {
        3 => Some(Rgb(single(0)?, single(1)?, single(2)?)),
        // Mostly see-through colours are overlays, not brand colours.
        4 if single(3)? >= 128 => Some(Rgb(single(0)?, single(1)?, single(2)?)),
        6 => Some(Rgb(pair(0)?, pair(2)?, pair(4)?)),
        8 if pair(6)? >= 128 => Some(Rgb(pair(0)?, pair(2)?, pair(4)?)),
        _ => None,
    }
}

/// `rgb(…)`/`rgba(…)`/`hsl(…)` with numbers in either comma or space form.
fn parse_function(name: &str, args: &str) -> Option<Rgb> {
    let parts: Vec<&str> = args.split([',', ' ', '/']).map(str::trim).filter(|p| !p.is_empty()).collect();
    if parts.len() < 3 {
        return None;
    }
    if let Some(alpha) = parts.get(3) {
        let alpha = alpha.strip_suffix('%').map_or_else(|| alpha.parse::<f64>().ok(), |p| p.parse::<f64>().ok().map(|v| v / 100.0))?;
        if alpha < 0.5 {
            return None;
        }
    }
    let number = |text: &str, scale: f64| -> Option<f64> {
        match text.strip_suffix('%') {
            Some(percent) => percent.parse::<f64>().ok().map(|v| v / 100.0 * scale),
            None => text.trim_end_matches("deg").parse().ok(),
        }
    };
    if name.starts_with("rgb") {
        let channel = |text: &str| number(text, 255.0).map(|v| v.clamp(0.0, 255.0).round() as u8);
        return Some(Rgb(channel(parts[0])?, channel(parts[1])?, channel(parts[2])?));
    }
    let hue = number(parts[0], 360.0)?.rem_euclid(360.0) / 360.0;
    let saturation = number(parts[1], 1.0)?.clamp(0.0, 1.0);
    let lightness = number(parts[2], 1.0)?.clamp(0.0, 1.0);
    let q = if lightness < 0.5 { lightness * (1.0 + saturation) } else { lightness + saturation - lightness * saturation };
    let p = 2.0 * lightness - q;
    let channel = |mut t: f64| {
        t = t.rem_euclid(1.0);
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round() as u8
    };
    Some(Rgb(channel(hue + 1.0 / 3.0), channel(hue), channel(hue - 1.0 / 3.0)))
}

/// Every colour written in a value, in order.
pub fn colours_in(value: &str) -> Vec<Rgb> {
    let mut found = Vec::new();
    let lower = value.to_ascii_lowercase();
    let mut at = 0;
    while at < lower.len() {
        let rest = &lower[at..];
        if rest.starts_with('#') {
            let len = rest[1..].find(|c: char| !c.is_ascii_hexdigit()).unwrap_or(rest.len() - 1);
            if let Some(rgb) = parse_hex(&rest[..=len]) {
                found.push(rgb);
            }
            at += len + 1;
            continue;
        }
        let function = ["rgba(", "rgb(", "hsla(", "hsl("].into_iter().find(|name| rest.starts_with(name));
        if let Some(name) = function {
            if let Some(len) = matching_paren(&rest[name.len() - 1..]) {
                if let Some(rgb) = parse_function(name, &rest[name.len()..name.len() - 1 + len]) {
                    found.push(rgb);
                }
                at += name.len() + len;
                continue;
            }
        }
        if rest.starts_with("white") && !rest[5..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-') && boundary_before(&lower, at) {
            found.push(Rgb(255, 255, 255));
        } else if rest.starts_with("black") && !rest[5..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-') && boundary_before(&lower, at) {
            found.push(Rgb(0, 0, 0));
        }
        at += rest.chars().next().map_or(1, char::len_utf8);
    }
    found
}

fn boundary_before(text: &str, at: usize) -> bool {
    at == 0 || !text[..at].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '-')
}

/// What a colour is used for, in words Claude and the fallback both read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Use {
    ThemeColour,
    PageBackground,
    PageText,
    HeaderBackground,
    Button,
    Link,
    Heading,
    BrandVariable,
    Background,
    Text,
    Border,
}

impl Use {
    pub fn words(self) -> &'static str {
        match self {
            Self::ThemeColour => "theme colour",
            Self::PageBackground => "page background",
            Self::PageText => "page text",
            Self::HeaderBackground => "header background",
            Self::Button => "buttons",
            Self::Link => "links",
            Self::Heading => "headings",
            Self::BrandVariable => "named brand variable",
            Self::Background => "backgrounds",
            Self::Text => "text",
            Self::Border => "borders and lines",
        }
    }

    fn weight(self) -> f64 {
        match self {
            Self::ThemeColour => 20.0,
            Self::PageBackground => 12.0,
            Self::PageText => 8.0,
            Self::BrandVariable => 8.0,
            Self::HeaderBackground | Self::Button => 5.0,
            Self::Link | Self::Heading => 3.0,
            Self::Background | Self::Text => 1.0,
            Self::Border => 0.3,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColourCandidate {
    pub rgb: Rgb,
    pub score: f64,
    pub uses: Vec<Use>,
}

impl ColourCandidate {
    pub fn hex(&self) -> String {
        self.rgb.hex()
    }

    pub fn has(&self, used: Use) -> bool {
        self.uses.contains(&used)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Background,
    Text,
    Border,
}

fn property_role(property: &str) -> Option<Role> {
    match property {
        "background" | "background-color" | "background-image" | "fill" => Some(Role::Background),
        "color" | "stroke" => Some(Role::Text),
        _ if property.starts_with("border") || property == "outline" || property == "box-shadow" || property.ends_with("-shadow") => Some(Role::Border),
        _ => None,
    }
}

fn selector_is(selector: &str, names: &[&str]) -> bool {
    selector.to_ascii_lowercase().split(',').any(|part| {
        let part = part.trim();
        let last = part.rsplit([' ', '>']).next().unwrap_or(part);
        names.iter().any(|name| last == *name || last.starts_with(&format!("{name}:")) || last.starts_with(&format!("{name}.")) || last.starts_with(&format!("{name}[")))
    })
}

fn selector_mentions(selector: &str, words: &[&str]) -> bool {
    let lower = selector.to_ascii_lowercase();
    words.iter().any(|word| lower.contains(word))
}

fn uses_for(selector: &str, role: Role) -> Use {
    let page = selector_is(selector, &["html", "body", ":root"]);
    match role {
        Role::Border => Use::Border,
        Role::Background if page => Use::PageBackground,
        Role::Text if page => Use::PageText,
        Role::Background if selector_is(selector, &["header", "nav"]) || selector_mentions(selector, &["header", "navbar"]) => Use::HeaderBackground,
        _ if selector_is(selector, &["button"]) || selector_mentions(selector, &["btn", "button", "cta"]) => Use::Button,
        Role::Text if selector_is(selector, &["a"]) => Use::Link,
        Role::Text if selector_is(selector, &["h1", "h2", "h3"]) => Use::Heading,
        Role::Background => Use::Background,
        Role::Text => Use::Text,
    }
}

const BRAND_WORDS: [&str; 8] = ["brand", "primary", "accent", "secondary", "theme", "main", "highlight", "key"];

/// Colours ranked by score; near-identical shades are merged into the stronger.
pub fn rank_colours(declarations: &[Declaration], theme_colour: Option<&str>) -> Vec<ColourCandidate> {
    let vars = variables(declarations);
    let mut tally: Vec<ColourCandidate> = Vec::new();
    let mut add = |rgb: Rgb, used: Use| {
        let score = used.weight();
        if let Some(found) = tally.iter_mut().find(|c| c.rgb.distance(rgb) < 14.0) {
            found.score += score;
            if !found.uses.contains(&used) {
                found.uses.push(used);
            }
        } else {
            tally.push(ColourCandidate { rgb, score, uses: vec![used] });
        }
    };
    if let Some(rgb) = theme_colour.and_then(|c| colours_in(c).into_iter().next()) {
        add(rgb, Use::ThemeColour);
    }
    for declaration in declarations.iter().filter(|d| !d.selector.starts_with('@')) {
        if let Some(name) = declaration.property.strip_prefix("--") {
            if BRAND_WORDS.iter().any(|word| name.contains(word)) && !name.contains("shadow") {
                for rgb in colours_in(&substitute(&declaration.value, &vars)) {
                    add(rgb, Use::BrandVariable);
                }
            }
            continue;
        }
        let Some(role) = property_role(&declaration.property) else { continue };
        let used = uses_for(&declaration.selector, role);
        for rgb in colours_in(&substitute(&declaration.value, &vars)) {
            add(rgb, used);
        }
    }
    tally.sort_by(|a, b| b.score.total_cmp(&a.score));
    tally
}

fn variables(declarations: &[Declaration]) -> HashMap<String, String> {
    declarations.iter().filter(|d| d.property.starts_with("--")).map(|d| (d.property.clone(), d.value.clone())).collect()
}

// --- fonts -------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct FontCandidate {
    pub family: String,
    pub score: f64,
    pub on_headings: bool,
    pub on_body: bool,
    /// Named in a Google Fonts link, so it is a real downloadable family.
    pub google: bool,
}

const GENERIC: [&str; 22] = [
    "serif", "sans-serif", "monospace", "cursive", "fantasy", "system-ui", "ui-sans-serif", "ui-serif", "ui-monospace", "ui-rounded",
    "-apple-system", "blinkmacsystemfont", "segoe ui", "helvetica neue", "helvetica", "arial", "apple color emoji", "segoe ui emoji",
    "segoe ui symbol", "noto color emoji", "inherit", "initial",
];

/// The first real family in a `font-family` list.
pub fn first_family(value: &str) -> Option<String> {
    value
        .split(',')
        .map(|family| family.trim().trim_matches(['"', '\'']).trim())
        .find(|family| !family.is_empty() && !GENERIC.contains(&family.to_ascii_lowercase().as_str()) && !family.starts_with("var("))
        .map(str::to_owned)
}

/// The family in a `font` shorthand (`700 18px/1.2 "Barlow", sans-serif`):
/// whatever follows the size.
pub fn shorthand_family(value: &str) -> Option<String> {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    let is_size = |t: &&str| t.starts_with(|c: char| c.is_ascii_digit() || c == '.') && t.contains(|c: char| c.is_ascii_alphabetic() || c == '%');
    let size_at = tokens.iter().position(is_size)?;
    let rest: Vec<&str> = tokens[size_at + 1..]
        .iter()
        .copied()
        .skip_while(|t| t.starts_with('/') || t.starts_with(|c: char| c.is_ascii_digit() || c == '.'))
        .collect();
    first_family(&rest.join(" "))
}

fn valid_family(name: &str) -> bool {
    name.len() <= 60 && name.chars().all(|c| c.is_alphanumeric() || " -'".contains(c)) && name.chars().any(char::is_alphabetic)
}

/// Fonts ranked by use; `google` lists families from Google Fonts links.
pub fn rank_fonts(declarations: &[Declaration], google: &[String]) -> Vec<FontCandidate> {
    let vars = variables(declarations);
    let mut tally: Vec<FontCandidate> = Vec::new();
    let mut add = |family: String, score: f64, headings: bool, body: bool, from_google: bool| {
        if !valid_family(&family) {
            return;
        }
        if let Some(found) = tally.iter_mut().find(|f| f.family.eq_ignore_ascii_case(&family)) {
            found.score += score;
            found.on_headings |= headings;
            found.on_body |= body;
            found.google |= from_google;
        } else {
            tally.push(FontCandidate { family, score, on_headings: headings, on_body: body, google: from_google });
        }
    };
    for family in google {
        add(family.clone(), 2.0, false, false, true);
    }
    for declaration in declarations {
        let is_font_var = declaration.property.starts_with("--") && declaration.property.contains("font") && !declaration.property.contains("size") && !declaration.property.contains("weight");
        let value = substitute(&declaration.value, &vars);
        let family = match declaration.property.as_str() {
            "font-family" => first_family(&value),
            "font" => shorthand_family(&value),
            _ if is_font_var => first_family(&value),
            _ => continue,
        };
        let Some(family) = family else { continue };
        if declaration.selector == "@font-face" {
            add(family, 0.5, false, false, false);
            continue;
        }
        let name = declaration.property.as_str();
        let headings = selector_is(&declaration.selector, &["h1", "h2", "h3", "h4"]) || selector_mentions(&declaration.selector, &["heading", "title", "display", "hero"]) || (is_font_var && ["head", "display", "title"].iter().any(|w| name.contains(w)));
        let body = selector_is(&declaration.selector, &["html", "body", "p"]) || (is_font_var && ["body", "base", "text", "sans"].iter().any(|w| name.contains(w)));
        let score = if body { 6.0 } else if headings { 5.0 } else { 1.0 };
        add(family, score, headings, body, false);
    }
    tally.sort_by(|a, b| b.score.total_cmp(&a.score));
    tally
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSS: &str = r#"
@import url("x.css");
/* :root { --brand: #00FF00; } */
:root { --brand-primary: #E4572E; --ink: #1B1B1B; --font-display: "Playfair Display", serif; --space: 4px; }
html, body { background: #FAF7F2; color: var(--ink); font-family: Inter, -apple-system, sans-serif; }
@media (max-width: 600px) { .hero h1 { color: #e4572f; font-family: var(--font-display); } }
@font-face { font-family: "Ignored"; src: url(a.woff2); }
.btn-primary { background-color: rgb(228 87 46 / 100%); border: 1px solid #ddd; }
.overlay { background: rgba(0,0,0,.4); }
a:hover { color: hsl(210, 100%, 40%); }
"#;

    #[test]
    fn declarations_are_read_through_media_rules() {
        let all = declarations(CSS);
        assert!(all.iter().any(|d| d.selector == ".hero h1" && d.property == "color"));
        assert!(all.iter().any(|d| d.selector == "@font-face" && d.value.contains("Ignored")), "font-face keeps only its family");
        assert!(!all.iter().any(|d| d.property == "src"));
        assert!(!all.iter().any(|d| d.value.contains("#00FF00")), "comments left out");
        assert_eq!(all[0].selector, ":root");
    }

    #[test]
    fn colours_are_parsed_in_every_form() {
        assert_eq!(colours_in("linear-gradient(#fff, #00B4D8 50%)"), vec![Rgb(255, 255, 255), Rgb(0, 180, 216)]);
        assert_eq!(colours_in("rgb(10, 20, 30)"), vec![Rgb(10, 20, 30)]);
        assert_eq!(colours_in("rgba(0,0,0,.2)"), vec![], "mostly see-through is not a brand colour");
        assert_eq!(colours_in("hsl(0 100% 50%)"), vec![Rgb(255, 0, 0)]);
        assert_eq!(colours_in("1px solid black"), vec![Rgb(0, 0, 0)]);
        assert_eq!(colours_in("blackish whitesmoke"), vec![]);
        assert_eq!(colours_in("#12345680"), vec![Rgb(0x12, 0x34, 0x56)]);
        assert_eq!(colours_in("#1234"), vec![], "alpha 0x44 is see-through");
    }

    #[test]
    fn page_colours_rank_above_borders() {
        let ranked = rank_colours(&declarations(CSS), Some("#E4572E"));
        let top: Vec<String> = ranked.iter().take(3).map(ColourCandidate::hex).collect();
        assert_eq!(top, vec!["#E4572E", "#FAF7F2", "#1B1B1B"]);
        let brand = &ranked[0];
        assert!(brand.has(Use::ThemeColour) && brand.has(Use::BrandVariable) && brand.has(Use::Heading) && brand.has(Use::Button), "{brand:?}");
        assert!(ranked.iter().find(|c| c.hex() == "#FAF7F2").unwrap().has(Use::PageBackground));
        assert!(ranked.iter().find(|c| c.hex() == "#1B1B1B").unwrap().has(Use::PageText), "var() is followed");
        assert!(!ranked.iter().any(|c| c.hex() == "#000000"), "the see-through overlay is skipped");
        assert_eq!(ranked.last().unwrap().hex(), "#DDDDDD");
    }

    #[test]
    fn fonts_are_ranked_with_their_roles() {
        let ranked = rank_fonts(&declarations(CSS), &["Playfair Display".into(), "Inter".into()]);
        let inter = ranked.iter().find(|f| f.family == "Inter").unwrap();
        let playfair = ranked.iter().find(|f| f.family == "Playfair Display").unwrap();
        assert!(inter.on_body && inter.google && !inter.on_headings);
        assert!(playfair.on_headings && playfair.google);
        let loaded = ranked.iter().find(|f| f.family == "Ignored").unwrap();
        assert!(!loaded.on_body && !loaded.on_headings && loaded.score < 1.0, "a loaded web font counts a little");
        assert!(!ranked.iter().any(|f| f.family == "-apple-system"));
        let shorthand = rank_fonts(&declarations(":root{--cond:'Barlow Condensed',sans-serif} h1{font:700 18px/1.1 var(--cond)} body{font: 400 1rem / 1.5 Barlow, sans-serif}"), &[]);
        assert_eq!(shorthand.iter().map(|f| (f.family.as_str(), f.on_headings, f.on_body)).collect::<Vec<_>>(), vec![("Barlow", false, true), ("Barlow Condensed", true, false)]);
        assert_eq!(shorthand_family("italic bold 12px/30px Georgia, serif").as_deref(), Some("Georgia"));
        assert_eq!(shorthand_family("inherit"), None);
        assert_eq!(first_family("'Segoe UI', Roboto, sans-serif").as_deref(), Some("Roboto"));
        assert_eq!(first_family("system-ui, sans-serif"), None);
    }
}
