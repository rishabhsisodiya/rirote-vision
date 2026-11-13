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
/// Photos, cut-outs and 3D avatars together.
pub const MAX_ASSETS: usize = 30;
const MAX_PHOTO_BYTES: u64 = 20 * 1024 * 1024;
const MAX_AVATAR_BYTES: u64 = 100 * 1024 * 1024;
const MAX_LABEL: usize = 60;
const MAX_NOTE: usize = 200;
/// Where a kit keeps its assets, inside its own folder.
const ASSETS_FOLDER: &str = "assets";

/// What a kit asset is.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AssetKind {
    Photo,
    /// A photo with its background removed (transparent PNG).
    Cutout,
    /// A 3D model in one binary glTF (`.glb`) file.
    Avatar3d,
}

/// A photo, cut-out or 3D avatar kept in a brand kit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    pub kind: AssetKind,
    /// Full path of the file in the kit's folder.
    pub file: String,
    /// A short name: "Yuvraj".
    pub label: String,
    /// When to use it: "founder, use on the intro".
    pub note: String,
    /// For a cut-out, the id of the photo it was made from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cutout_of: Option<String>,
}

impl Asset {
    fn file_name(&self) -> String {
        Path::new(&self.file).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }
}

/// What a file is, judged by its first bytes rather than its name.
fn sniff(source: &Path) -> AppResult<AssetKind> {
    use std::io::Read;
    let mut head = [0u8; 12];
    let mut file = std::fs::File::open(source).map_err(|_| AppError::NotFound("That file is gone.".into()))?;
    let read = file.read(&mut head)?;
    let head = &head[..read];
    let extension = source.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let image = head.starts_with(b"\x89PNG\r\n\x1a\n")
        || head.starts_with(&[0xFF, 0xD8, 0xFF])
        || (head.len() == 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP");
    let glb = head.starts_with(b"glTF");
    match extension.as_str() {
        "png" | "jpg" | "jpeg" | "webp" if image => Ok(AssetKind::Photo),
        "glb" if glb => Ok(AssetKind::Avatar3d),
        "gltf" => Err(AppError::InvalidInput("Export the 3D model as a single .glb file; a .gltf points to other files that would be left behind.".into())),
        "png" | "jpg" | "jpeg" | "webp" | "glb" => Err(AppError::InvalidInput("That file's contents do not match its name.".into())),
        _ => Err(AppError::InvalidInput("Add photos as PNG, JPEG or WebP, and 3D avatars as .glb.".into())),
    }
}

/// One line of plain text, with no characters that could break out of a prompt line.
fn clean_line(text: &str, max: usize, what: &str) -> AppResult<String> {
    let line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let line: String = line.chars().filter(|c| !c.is_control()).collect();
    if line.chars().count() > max {
        return Err(AppError::InvalidInput(format!("Keep the {what} under {max} characters.")));
    }
    Ok(line)
}

/// A safe file name stem: lowercase letters, digits and hyphens.
fn file_stem(source: &Path) -> String {
    let raw = source.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mut stem = String::new();
    for c in raw.chars() {
        if stem.len() >= 40 {
            break;
        }
        if c.is_ascii_alphanumeric() {
            stem.push(c);
        } else if !stem.is_empty() && !stem.ends_with('-') {
            stem.push('-');
        }
    }
    let stem = stem.trim_end_matches('-').to_owned();
    // `logo.*` is the kit logo's name in a project's assets/brand folder.
    match stem.as_str() {
        "" => "asset".into(),
        "logo" => "logo-asset".into(),
        _ => stem,
    }
}

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
    /// Photos, cut-outs and 3D avatars. Changed only through the asset methods.
    pub assets: Vec<Asset>,
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
            assets: Vec::new(),
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
        let describe = |asset: &Asset| {
            let mut line = format!("assets/brand/{}", asset.file_name());
            if !asset.label.is_empty() {
                line.push_str(&format!(" — {}", asset.label));
            }
            if !asset.note.is_empty() {
                line.push_str(&format!(" ({})", asset.note));
            }
            line
        };
        let photos: Vec<&Asset> = self.assets.iter().filter(|a| a.kind == AssetKind::Photo).collect();
        if !photos.is_empty() {
            lines.push("- Photos (already in the project):".into());
            for photo in photos {
                let mut line = format!("  - {}", describe(photo));
                if let Some(cutout) = self.assets.iter().find(|a| a.cutout_of.as_deref() == Some(photo.id.as_str())) {
                    line.push_str(&format!("; background removed: assets/brand/{}", cutout.file_name()));
                }
                lines.push(line);
            }
        }
        let avatars: Vec<&Asset> = self.assets.iter().filter(|a| a.kind == AssetKind::Avatar3d).collect();
        if !avatars.is_empty() {
            lines.push("- 3D avatars (already in the project; .glb, show them with Three.js):".into());
            for avatar in avatars {
                lines.push(format!("  - {}", describe(avatar)));
            }
        }
        lines.join("\n")
    }
}

fn asset_id(file: &Path) -> String {
    let seed = format!("{}{}", file.display(), SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    format!("asset-{:012x}", ids::fnv1a(seed.as_bytes()) & 0xffff_ffff_ffff)
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
            kit.assets = Vec::new();
        } else {
            let saved = self.get(&kit.id)?;
            kit.logo = saved.logo;
            kit.assets = saved.assets;
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

    fn write(&self, mut kit: Kit) -> AppResult<Kit> {
        kit.updated_at = now();
        store::write_atomic(&self.dir(&kit.id)?.join("kit.json"), &kit)?;
        Ok(kit)
    }

    /// Copies a photo or `.glb` avatar into the kit, checked by its contents.
    pub fn add_asset(&self, id: &str, source: &Path, label: &str, note: &str) -> AppResult<Kit> {
        let kind = sniff(source)?;
        let size = std::fs::metadata(source).map(|m| m.len()).unwrap_or(0);
        let (limit, what) = match kind {
            AssetKind::Avatar3d => (MAX_AVATAR_BYTES, "3D avatar under 100 MB"),
            _ => (MAX_PHOTO_BYTES, "photo under 20 MB"),
        };
        if size > limit {
            return Err(AppError::InvalidInput(format!("Keep each {what}.")));
        }
        let label = clean_line(label, MAX_LABEL, "label")?;
        let note = clean_line(note, MAX_NOTE, "note")?;
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut kit = self.get(id)?;
        if kit.assets.len() >= MAX_ASSETS {
            return Err(AppError::InvalidInput(format!("A kit holds up to {MAX_ASSETS} assets. Remove one first.")));
        }
        let folder = self.dir(id)?.join(ASSETS_FOLDER);
        std::fs::create_dir_all(&folder)?;
        let extension = source.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        let target = self.free_asset_name(&kit, &folder, &file_stem(source), &extension);
        std::fs::copy(source, &target)?;
        kit.assets.push(Asset {
            id: asset_id(&target),
            kind,
            file: target.to_string_lossy().into_owned(),
            label,
            note,
            cutout_of: None,
        });
        self.write(kit)
    }

    /// A file name in the kit's assets folder that no asset uses yet.
    /// Names are kept unique across the whole kit, because a project gets
    /// them all in one folder.
    fn free_asset_name(&self, kit: &Kit, folder: &Path, stem: &str, extension: &str) -> PathBuf {
        let taken = |name: &str| kit.assets.iter().any(|a| a.file_name() == name) || folder.join(name).exists();
        let mut name = format!("{stem}.{extension}");
        let mut number = 2;
        while taken(&name) {
            name = format!("{stem}-{number}.{extension}");
            number += 1;
        }
        folder.join(name)
    }

    /// Changes an asset's label and note.
    pub fn update_asset(&self, id: &str, asset: &str, label: &str, note: &str) -> AppResult<Kit> {
        let label = clean_line(label, MAX_LABEL, "label")?;
        let note = clean_line(note, MAX_NOTE, "note")?;
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut kit = self.get(id)?;
        let found = kit.assets.iter_mut().find(|a| a.id == asset).ok_or_else(|| AppError::NotFound("That asset is no longer in the kit.".into()))?;
        found.label = label;
        found.note = note;
        self.write(kit)
    }

    /// Removes an asset and its file. Removing a photo removes its cut-out too.
    /// Projects keep the copies they already have.
    pub fn remove_asset(&self, id: &str, asset: &str) -> AppResult<Kit> {
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut kit = self.get(id)?;
        if !kit.assets.iter().any(|a| a.id == asset) {
            return Err(AppError::NotFound("That asset is no longer in the kit.".into()));
        }
        let (gone, kept): (Vec<Asset>, Vec<Asset>) =
            kit.assets.into_iter().partition(|a| a.id == asset || a.cutout_of.as_deref() == Some(asset));
        for old in gone {
            let _ = std::fs::remove_file(&old.file);
        }
        kit.assets = kept;
        self.write(kit)
    }

    /// The photo to cut out and where its cut-out goes. The engine writes the
    /// file; `add_cutout` then records it.
    pub fn cutout_plan(&self, id: &str, photo: &str) -> AppResult<(PathBuf, PathBuf)> {
        let kit = self.get(id)?;
        let source = kit.assets.iter().find(|a| a.id == photo).ok_or_else(|| AppError::NotFound("That photo is no longer in the kit.".into()))?;
        if source.kind != AssetKind::Photo {
            return Err(AppError::InvalidInput("Only photos can have their background removed.".into()));
        }
        let folder = self.dir(id)?.join(ASSETS_FOLDER);
        let stem = Path::new(&source.file).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "photo".into());
        // A scratch name first, so a failed or cancelled run leaves nothing behind.
        let output = folder.join(format!(".{stem}-cutout-{}.png", std::process::id()));
        Ok((PathBuf::from(&source.file), output))
    }

    /// Records a cut-out the engine made from `photo`, replacing any earlier one.
    pub fn add_cutout(&self, id: &str, photo: &str, made: &Path) -> AppResult<Kit> {
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut kit = self.get(id)?;
        let Some(source) = kit.assets.iter().find(|a| a.id == photo).cloned() else {
            let _ = std::fs::remove_file(made);
            return Err(AppError::NotFound("The photo was removed while its cut-out was being made.".into()));
        };
        for old in kit.assets.iter().filter(|a| a.cutout_of.as_deref() == Some(photo)) {
            let _ = std::fs::remove_file(&old.file);
        }
        kit.assets.retain(|a| a.cutout_of.as_deref() != Some(photo));
        let folder = self.dir(id)?.join(ASSETS_FOLDER);
        let stem = format!("{}-cutout", Path::new(&source.file).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
        let target = self.free_asset_name(&kit, &folder, &stem, "png");
        std::fs::rename(made, &target)?;
        kit.assets.push(Asset {
            id: asset_id(&target),
            kind: AssetKind::Cutout,
            file: target.to_string_lossy().into_owned(),
            label: if source.label.is_empty() { String::new() } else { format!("{} (cut-out)", source.label) },
            note: source.note.clone(),
            cutout_of: Some(photo.to_owned()),
        });
        self.write(kit)
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        let dir = self.dir(id)?;
        if dir.is_dir() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    /// Chooses the project's kit (or none) and copies its logo and assets
    /// into `assets/brand/`, so the composition can use them.
    pub fn use_in(&self, project: &Path, id: Option<&str>) -> AppResult<Option<Kit>> {
        let kit = id.map(|id| self.get(id)).transpose()?;
        if let Some(kit) = &kit {
            let files = kit.logo.iter().cloned().chain(kit.assets.iter().map(|a| a.file.clone()));
            let folder = media::assets_dir(project).join("brand");
            for file in files {
                std::fs::create_dir_all(&folder)?;
                let name = Path::new(&file).file_name().ok_or_else(|| AppError::NotFound("A kit file is missing.".into()))?.to_owned();
                std::fs::copy(&file, folder.join(&name))
                    .map_err(|_| AppError::NotFound(format!("{} is missing from the kit's folder.", name.to_string_lossy())))?;
            }
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

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n0000";

    #[test]
    fn a_kit_saved_before_assets_existed_still_loads() {
        let old = r##"{"id":"kit-1","name":"Old","colours":[{"name":"Accent","hex":"#FF6A2B"}],"headingFont":"Barlow","bodyFont":"Barlow","logo":null,"tone":"","website":"","updatedAt":1}"##;
        let kit: Kit = serde_json::from_str(old).unwrap();
        assert_eq!(kit.name, "Old");
        assert!(kit.assets.is_empty());
    }

    #[test]
    fn photos_and_avatars_are_added_checked_copied_and_removed() {
        let data = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let kits = BrandKits::new(data.path());
        let saved = kits.save(kit()).unwrap();
        let file = |name: &str, bytes: &[u8]| {
            let path = data.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        };

        let with_photo = kits.add_asset(&saved.id, &file("Yuvraj Singh.JPG", &[0xFF, 0xD8, 0xFF, 0xE0]), " Yuvraj ", "founder,\nuse on intro").unwrap();
        let photo = with_photo.assets[0].clone();
        assert_eq!(photo.kind, AssetKind::Photo);
        assert!(photo.file.ends_with("assets/yuvraj-singh.jpg"), "{}", photo.file);
        assert_eq!((photo.label.as_str(), photo.note.as_str()), ("Yuvraj", "founder, use on intro"));
        let with_avatar = kits.add_asset(&saved.id, &file("me.glb", b"glTF\x02\0\0\0"), "Avatar", "").unwrap();
        assert_eq!(with_avatar.assets[1].kind, AssetKind::Avatar3d);
        // Same name again gets its own file.
        let twice = kits.add_asset(&saved.id, &file("yuvraj-singh.png", PNG), "", "").unwrap();
        assert!(twice.assets[2].file.ends_with("yuvraj-singh.png"));
        let named_logo = kits.add_asset(&saved.id, &file("logo.png", PNG), "", "").unwrap();
        assert!(named_logo.assets[3].file.ends_with("logo-asset.png"), "never clashes with the kit logo");

        // Saving the form keeps the assets it does not send.
        let resaved = kits.save(Kit { assets: Vec::new(), ..named_logo.clone() }).unwrap();
        assert_eq!(resaved.assets.len(), 4);

        // A cut-out from the engine is recorded beside its photo.
        let (source, scratch) = kits.cutout_plan(&saved.id, &photo.id).unwrap();
        assert_eq!(source.to_string_lossy(), photo.file);
        std::fs::write(&scratch, PNG).unwrap();
        let cut = kits.add_cutout(&saved.id, &photo.id, &scratch).unwrap();
        let cutout = cut.assets.iter().find(|a| a.kind == AssetKind::Cutout).unwrap().clone();
        assert!(cutout.file.ends_with("yuvraj-singh-cutout.png") && !scratch.exists());
        assert_eq!(cutout.label, "Yuvraj (cut-out)");
        // Making it again replaces it rather than adding a second.
        std::fs::write(&scratch, PNG).unwrap();
        let again = kits.add_cutout(&saved.id, &photo.id, &scratch).unwrap();
        assert_eq!(again.assets.iter().filter(|a| a.kind == AssetKind::Cutout).count(), 1);
        assert_eq!(kits.cutout_plan(&saved.id, &with_avatar.assets[1].id).unwrap_err().code(), "invalid_input");

        let brief = again.brief();
        assert!(brief.contains("assets/brand/yuvraj-singh.jpg — Yuvraj (founder, use on intro); background removed: assets/brand/yuvraj-singh-cutout.png"), "{brief}");
        assert!(brief.contains("3D avatars") && brief.contains("assets/brand/me.glb — Avatar"));

        kits.use_in(project.path(), Some(&saved.id)).unwrap();
        for name in ["yuvraj-singh.jpg", "yuvraj-singh-cutout.png", "me.glb", "logo-asset.png"] {
            assert!(media::assets_dir(project.path()).join("brand").join(name).is_file(), "{name}");
        }

        // Removing the photo takes its cut-out with it; the project keeps its copies.
        let removed = kits.remove_asset(&saved.id, &photo.id).unwrap();
        assert_eq!(removed.assets.len(), 3);
        assert!(!Path::new(&photo.file).exists() && !Path::new(&cutout.file).exists());
        assert!(media::assets_dir(project.path()).join("brand/yuvraj-singh.jpg").is_file());
        assert_eq!(kits.remove_asset(&saved.id, &photo.id).unwrap_err().code(), "not_found");
        // A cut-out for a photo removed meanwhile is thrown away.
        std::fs::write(&scratch, PNG).unwrap();
        assert_eq!(kits.add_cutout(&saved.id, &photo.id, &scratch).unwrap_err().code(), "not_found");
        assert!(!scratch.exists());
    }

    #[test]
    fn asset_files_are_judged_by_their_contents() {
        let data = tempfile::tempdir().unwrap();
        let kits = BrandKits::new(data.path());
        let saved = kits.save(kit()).unwrap();
        let add = |name: &str, bytes: &[u8]| {
            let path = data.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            kits.add_asset(&saved.id, &path, "", "").map(|_| ())
        };
        assert_eq!(add("fake.png", b"<html>").unwrap_err().code(), "invalid_input");
        assert_eq!(add("scene.gltf", b"{}").unwrap_err().code(), "invalid_input");
        assert_eq!(add("notes.txt", b"hello").unwrap_err().code(), "invalid_input");
        assert_eq!(add("ok.webp", b"RIFF\0\0\0\0WEBP").map(|_| "ok").unwrap(), "ok");
        let long = "x".repeat(MAX_NOTE + 1);
        let path = data.path().join("a.png");
        std::fs::write(&path, PNG).unwrap();
        assert_eq!(kits.add_asset(&saved.id, &path, "", &long).unwrap_err().code(), "invalid_input");
        assert_eq!(kits.add_asset(&saved.id, Path::new("/nope/gone.png"), "", "").unwrap_err().code(), "not_found");
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
