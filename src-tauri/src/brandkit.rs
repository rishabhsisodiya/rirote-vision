//! Brand kits: a business's colours, fonts, logo and tone, kept once in the
//! app's folder and chosen per project. The assistant is handed the kit when
//! a video is made or restyled.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::{ids, media, store};

const MAX_COLOURS: usize = 8;
const LOGO_TYPES: [&str; 5] = ["svg", "png", "jpg", "jpeg", "webp"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Colour {
    /// What it is for: "Background", "Accent", "Text".
    pub name: String,
    pub hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Kit {
    pub id: String,
    pub name: String,
    pub colours: Vec<Colour>,
    /// Font family names, as Google Fonts or the system knows them.
    pub heading_font: String,
    pub body_font: String,
    /// The logo's full path inside the kit's folder, once one is added.
    pub logo: Option<String>,
    /// How the brand talks, in a sentence or two.
    pub tone: String,
    pub website: String,
    pub updated_at: u64,
}

impl Default for Kit {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            colours: Vec::new(),
            heading_font: String::new(),
            body_font: String::new(),
            logo: None,
            tone: String::new(),
            website: String::new(),
            updated_at: 0,
        }
    }
}

/// What a project remembers about itself, in `.rirote/project.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectSettings {
    pub brand_kit: Option<String>,
}

pub fn project_file(project: &Path) -> PathBuf {
    project.join(".rirote").join("project.json")
}

fn valid_hex(hex: &str) -> bool {
    let digits = hex.strip_prefix('#').unwrap_or("");
    matches!(digits.len(), 3 | 6) && digits.chars().all(|c| c.is_ascii_hexdigit())
}

fn valid_font(name: &str) -> bool {
    name.len() <= 60 && name.chars().all(|c| c.is_alphanumeric() || " -'".contains(c))
}

impl Kit {
    fn validate(&self) -> AppResult<()> {
        if self.name.trim().is_empty() || self.name.len() > 60 {
            return Err(AppError::InvalidInput("Give the kit a name of up to 60 characters.".into()));
        }
        if self.colours.len() > MAX_COLOURS {
            return Err(AppError::InvalidInput(format!("A kit holds up to {MAX_COLOURS} colours.")));
        }
        if let Some(bad) = self.colours.iter().find(|colour| !valid_hex(&colour.hex)) {
            return Err(AppError::InvalidInput(format!("“{}” is not a colour; use a hex value like #00B4D8.", bad.hex)));
        }
        if !valid_font(&self.heading_font) || !valid_font(&self.body_font) {
            return Err(AppError::InvalidInput("Font names are letters, numbers, spaces and hyphens.".into()));
        }
        if self.tone.len() > 600 || self.website.len() > 200 {
            return Err(AppError::InvalidInput("Keep the tone under 600 characters and the website under 200.".into()));
        }
        Ok(())
    }

    /// The kit as the assistant reads it.
    pub fn brief(&self) -> String {
        let mut lines = vec![format!("Brand kit “{}”:", self.name)];
        if !self.colours.is_empty() {
            let colours: Vec<String> = self.colours.iter().map(|c| format!("{} {}", c.name, c.hex.to_uppercase())).collect();
            lines.push(format!("- Colours: {}.", colours.join(", ")));
        }
        if !self.heading_font.is_empty() || !self.body_font.is_empty() {
            lines.push(format!(
                "- Fonts: headings in {}, body text in {}.",
                if self.heading_font.is_empty() { "the body font" } else { &self.heading_font },
                if self.body_font.is_empty() { "the heading font" } else { &self.body_font }
            ));
        }
        if let Some(logo) = &self.logo {
            let name = Path::new(logo).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            lines.push(format!("- Logo: assets/brand/{name} (already in the project)."));
        }
        if !self.tone.trim().is_empty() {
            lines.push(format!("- Tone: {}", self.tone.trim()));
        }
        if !self.website.trim().is_empty() {
            lines.push(format!("- Website: {}", self.website.trim()));
        }
        lines.join("\n")
    }
}

pub struct BrandKits {
    root: PathBuf,
    lock: Mutex<()>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |time| time.as_secs())
}

impl BrandKits {
    pub fn new(data_dir: &Path) -> Self {
        Self { root: data_dir.join("brand-kits"), lock: Mutex::new(()) }
    }

    fn dir(&self, id: &str) -> AppResult<PathBuf> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(AppError::InvalidInput("That is not a brand kit.".into()));
        }
        Ok(self.root.join(id))
    }

    pub fn list(&self) -> Vec<Kit> {
        let Ok(entries) = std::fs::read_dir(&self.root) else { return Vec::new() };
        let mut kits: Vec<Kit> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| serde_json::from_str(&std::fs::read_to_string(entry.path().join("kit.json")).ok()?).ok())
            .collect();
        kits.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        kits
    }

    pub fn get(&self, id: &str) -> AppResult<Kit> {
        let text = std::fs::read_to_string(self.dir(id)?.join("kit.json")).map_err(|_| AppError::NotFound("That brand kit is gone.".into()))?;
        serde_json::from_str(&text).map_err(|error| AppError::Io(error.to_string()))
    }

    /// Saves a new kit (empty id) or an existing one. The logo is set with `set_logo`.
    pub fn save(&self, mut kit: Kit) -> AppResult<Kit> {
        kit.validate()?;
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        if kit.id.is_empty() {
            kit.id = format!("kit-{:012x}", ids::fnv1a(format!("{}{}", kit.name, SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos())).as_bytes()) & 0xffff_ffff_ffff);
            kit.logo = None;
        } else {
            kit.logo = self.get(&kit.id)?.logo;
        }
        for colour in &mut kit.colours {
            colour.hex = colour.hex.to_uppercase();
        }
        kit.updated_at = now();
        store::write_atomic(&self.dir(&kit.id)?.join("kit.json"), &kit)?;
        Ok(kit)
    }

    /// Copies a logo into the kit's folder, replacing any before it.
    pub fn set_logo(&self, id: &str, source: &Path) -> AppResult<Kit> {
        let extension = source.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if !LOGO_TYPES.contains(&extension.as_str()) {
            return Err(AppError::InvalidInput("A logo is an SVG, PNG, JPEG or WebP file.".into()));
        }
        if std::fs::metadata(source).map(|m| m.len()).unwrap_or(0) > 10 * 1024 * 1024 {
            return Err(AppError::InvalidInput("Keep the logo under 10 MB.".into()));
        }
        let mut kit = self.get(id)?;
        let dir = self.dir(id)?;
        if let Some(old) = &kit.logo {
            let _ = std::fs::remove_file(old);
        }
        let target = dir.join(format!("logo.{extension}"));
        std::fs::copy(source, &target)?;
        kit.logo = Some(target.to_string_lossy().into_owned());
        kit.updated_at = now();
        store::write_atomic(&dir.join("kit.json"), &kit)?;
        Ok(kit)
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        let dir = self.dir(id)?;
        if dir.is_dir() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    /// Chooses the project's kit (or none) and copies its logo into
    /// `assets/brand/`, so the composition can use it.
    pub fn use_in(&self, project: &Path, id: Option<&str>) -> AppResult<Option<Kit>> {
        let kit = id.map(|id| self.get(id)).transpose()?;
        if let Some(logo) = kit.as_ref().and_then(|kit| kit.logo.as_ref()) {
            let folder = media::assets_dir(project).join("brand");
            std::fs::create_dir_all(&folder)?;
            let name = Path::new(logo).file_name().ok_or_else(|| AppError::NotFound("The kit's logo is missing.".into()))?;
            std::fs::copy(logo, folder.join(name))?;
        }
        store::write_atomic(&project_file(project), &ProjectSettings { brand_kit: kit.as_ref().map(|kit| kit.id.clone()) })?;
        Ok(kit)
    }

    pub fn of_project(&self, project: &Path) -> Option<Kit> {
        let settings: ProjectSettings = store::read_or_default(&project_file(project));
        settings.brand_kit.and_then(|id| self.get(&id).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kit() -> Kit {
        Kit {
            name: "Rirote Global".into(),
            colours: vec![Colour { name: "Background".into(), hex: "#0a1628".into() }, Colour { name: "Accent".into(), hex: "#00b4d8".into() }],
            heading_font: "Bebas Neue".into(),
            body_font: "Inter".into(),
            tone: "Plain, confident, no hype.".into(),
            website: "rirote.com".into(),
            ..Kit::default()
        }
    }

    #[test]
    fn a_kit_is_saved_logo_added_used_and_deleted() {
        let data = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let kits = BrandKits::new(data.path());
        let saved = kits.save(kit()).unwrap();
        assert!(saved.id.starts_with("kit-"));
        assert_eq!(saved.colours[1].hex, "#00B4D8");
        let logo = data.path().join("mark.svg");
        std::fs::write(&logo, "<svg/>").unwrap();
        let with_logo = kits.set_logo(&saved.id, &logo).unwrap();
        assert!(with_logo.logo.as_deref().unwrap().ends_with("logo.svg"));
        // Saving again keeps the logo even though the form does not send it.
        let renamed = kits.save(Kit { name: "Rirote".into(), logo: None, ..with_logo.clone() }).unwrap();
        assert_eq!(renamed.logo, with_logo.logo);

        kits.use_in(project.path(), Some(&saved.id)).unwrap();
        assert!(media::assets_dir(project.path()).join("brand/logo.svg").is_file());
        assert_eq!(kits.of_project(project.path()).unwrap().name, "Rirote");
        let brief = kits.of_project(project.path()).unwrap().brief();
        assert!(brief.contains("Accent #00B4D8") && brief.contains("assets/brand/logo.svg") && brief.contains("headings in Bebas Neue"));

        kits.delete(&saved.id).unwrap();
        assert!(kits.list().is_empty());
        assert!(kits.of_project(project.path()).is_none(), "a deleted kit is no longer the project's");
    }

    #[test]
    fn bad_values_are_refused() {
        let kits = BrandKits::new(Path::new("/tmp/rv-kits"));
        let bad_colour = Kit { colours: vec![Colour { name: "x".into(), hex: "teal".into() }], ..kit() };
        assert_eq!(kits.save(bad_colour).unwrap_err().code(), "invalid_input");
        assert_eq!(kits.save(Kit { heading_font: "Inter; }".into(), ..kit() }).unwrap_err().code(), "invalid_input");
        assert_eq!(kits.save(Kit { name: " ".into(), ..kit() }).unwrap_err().code(), "invalid_input");
        assert!(kits.dir("../x").is_err());
    }
}
