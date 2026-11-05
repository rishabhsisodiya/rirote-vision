//! What a studied reference is made of, on disk and on the wire. Ported from
//! reelforge's models: the blueprint describes structure, never who is in it.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "1.0";

/// The steps of a study, in order.
pub const STEPS: [&str; 5] = ["download", "shots", "keyframes", "transcribe", "analyze"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceMeta {
    pub id: String,
    /// The link it came from, or the file's name.
    pub source: String,
    /// youtube, instagram, tiktok, x, web or file.
    pub platform: String,
    pub title: Option<String>,
    pub creator: Option<String>,
    pub duration: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Seconds since 1970.
    pub added_at: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StepState {
    Pending,
    Running,
    Done,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub name: String,
    pub state: StepState,
    pub seconds: Option<f64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub steps: Vec<Step>,
}

impl Default for Run {
    fn default() -> Self {
        Self {
            steps: STEPS
                .iter()
                .map(|name| Step { name: (*name).into(), state: StepState::Pending, seconds: None, error: None })
                .collect(),
        }
    }
}

impl Run {
    pub fn step(&mut self, name: &str) -> &mut Step {
        self.steps.iter_mut().find(|step| step.name == name).expect("a known step")
    }

    pub fn finished(&self) -> bool {
        self.steps.iter().all(|step| step.state == StepState::Done)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Shot {
    pub index: usize,
    pub start: f64,
    pub end: f64,
    pub duration: f64,
    /// Relative to the reference's folder.
    pub keyframe: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShotList {
    pub video_duration: f64,
    pub fps: f64,
    pub threshold: f64,
    pub shots: Vec<Shot>,
}

/// A word the engine heard, with its time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

// --- what the analyst returns -----------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ShotAnalysis {
    pub index: usize,
    pub shot_type: String,
    pub camera_motion: String,
    pub subject_pose: String,
    pub subject_action: String,
    #[serde(default)]
    pub on_screen_text: Option<String>,
    pub product_visible: bool,
    pub emotion: String,
    pub energy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct Hook {
    #[serde(default = "three")]
    pub window_seconds: f64,
    pub hook_type: String,
    pub analysis: String,
    #[serde(default)]
    pub spoken: Option<String>,
    pub visual: String,
}

fn three() -> f64 {
    3.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct Section {
    pub role: String,
    pub start: f64,
    pub end: f64,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct Music {
    pub mood: String,
    pub tempo: String,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct CaptionStyle {
    pub present: bool,
    pub position: String,
    pub style: String,
    #[serde(default)]
    pub animation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct Analysis {
    pub hook: Hook,
    pub structure: Vec<Section>,
    pub shots: Vec<ShotAnalysis>,
    pub music: Music,
    pub caption_style: CaptionStyle,
    pub summary: String,
}

// --- the blueprint ---------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pacing {
    pub total_duration: f64,
    pub shot_count: usize,
    pub avg_shot_length: f64,
    pub min_shot_length: f64,
    pub max_shot_length: f64,
    pub cuts_per_10s: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintShot {
    #[serde(flatten)]
    pub analysis: ShotAnalysis,
    pub start: f64,
    pub end: f64,
    pub duration: f64,
    pub keyframe: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Blueprint {
    pub schema_version: String,
    pub reference_id: String,
    pub source: String,
    pub language: Option<String>,
    pub model: String,
    pub created_at: u64,
    pub summary: String,
    pub hook: Hook,
    pub structure: Vec<Section>,
    pub pacing: Pacing,
    pub music: Music,
    pub caption_style: CaptionStyle,
    pub shots: Vec<BlueprintShot>,
}

/// One entry in the library, as the window lists it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    #[serde(flatten)]
    pub meta: ReferenceMeta,
    pub steps: Vec<Step>,
    /// queued, running, ready or failed.
    pub status: &'static str,
    /// The first shot's still, as a full path, once there is one.
    pub poster: Option<String>,
    /// The folder the reference's files are in.
    pub folder: String,
}

/// A search result: not downloaded until it is picked.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub id: String,
    pub url: String,
    pub title: String,
    pub creator: Option<String>,
    pub duration: Option<f64>,
    pub views: Option<u64>,
    pub platform: String,
    /// A cached copy of the thumbnail, as a full path.
    pub thumbnail: Option<String>,
}
