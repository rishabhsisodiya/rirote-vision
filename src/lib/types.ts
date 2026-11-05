// Shapes shared with the Rust commands in src-tauri/src.

export interface ProjectSummary {
  path: string;
  name: string;
}

export interface RecentProject extends ProjectSummary {
  /** Seconds since 1970. */
  openedAt: number;
  /** The folder was moved or deleted since it was last opened. */
  missing: boolean;
}

export interface ProjectInfo {
  name: string;
  width: number;
  height: number;
  duration: number;
  elements: number;
  tracks: number;
}

export type Resolution = "landscape" | "portrait" | "square";

export interface NewProjectInput {
  parent: string;
  name: string;
  resolution: Resolution;
}

export type CheckStatus = "ok" | "warning" | "missing";

/** A repair Check Setup can carry out itself. */
export type SetupAction = "install-engine" | "install-voice" | "install-skills" | "install-ytdlp";

export interface SetupCheck {
  id: string;
  label: string;
  status: CheckStatus;
  detail: string;
  hint: string | null;
  required: boolean;
  action: SetupAction | null;
}

export interface SetupReport {
  ready: boolean;
  checks: SetupCheck[];
}

export interface PreviewSession {
  projectPath: string;
  /** The compiled composition, playable in a frame. */
  url: string;
  /** Changes whenever the project's files change. */
  signature: string;
  /** This view's hold on the server, given back when the view goes away. */
  lease: number;
}

export interface PreviewChanged {
  projectPath: string;
  signature: string;
}

export type ClipKind = "video" | "graphics" | "captions" | "audio";

export interface Clip {
  id: string;
  /** How the engine addresses the clip, for example `#intro`. */
  ref: string;
  label: string | null;
  kind: ClipKind | string;
  track: number;
  /** Seconds on the main timeline. */
  start: number;
  end: number;
  src: string | null;
  /** How loud it plays, 1 being as recorded. Null for clips without sound. */
  volume: number | null;
  /** How fast its media plays, 1 being as recorded. Null for clips without media. */
  rate: number | null;
  /** Where in its media file the clip starts, in seconds. */
  mediaStart: number | null;
  /** Plays a media file. */
  media: boolean;
  /** Clips inside a nested composition are shown but not edited. */
  editable: boolean;
}

export interface Timeline {
  duration: number;
  clips: Clip[];
}

export interface TimelineState {
  timeline: Timeline;
  canUndo: boolean;
  canRedo: boolean;
}

export type EditOp =
  | { type: "move"; clip: string; start: number }
  | { type: "trimStart"; clip: string; start: number }
  | { type: "trimEnd"; clip: string; end: number }
  | { type: "split"; clip: string; time: number }
  | { type: "delete"; clip: string }
  | { type: "duplicate"; clip: string }
  | { type: "place"; clip: string; start: number; track: number }
  | { type: "setVolume"; clip: string; volume: number }
  | { type: "setRate"; clip: string; rate: number };

export interface ChatMessage {
  role: "user" | "assistant" | "tool" | "notice";
  text: string;
  /** For a tool message: the tool's name and the id of its call. */
  tool?: string;
  id?: string;
  /** For a tool message: whether it worked, once that is known. */
  ok?: boolean;
}

export interface PermissionRule {
  tool: string;
  content: string;
}

/** How much the assistant may do in a project without asking. */
export type TrustLevel = "ask" | "trusted" | "full";

export interface ChatView {
  messages: ChatMessage[];
  /** A turn is running for this project right now. */
  busy: boolean;
  rules: PermissionRule[];
  /** Null until a level has been chosen for this project. */
  trust: TrustLevel | null;
}

export interface ApprovalRequest {
  requestId: string;
  tool: string;
  detail: string;
  /** What "always allow" would cover, when the assistant can say. */
  rule: string | null;
}

export type AgentEvent =
  | { kind: "textDelta"; text: string }
  | { kind: "text"; text: string }
  | { kind: "tool"; id: string; tool: string; detail: string }
  | { kind: "toolResult"; id: string; ok: boolean }
  | ({ kind: "approval" } & ApprovalRequest)
  | { kind: "finished"; ok: boolean; text: string }
  | { kind: "failed"; message: string }
  | { kind: "stopped" };

export type ApprovalAnswer = "allow" | "always" | "deny";

/** What the editor is showing when a request is sent. */
export interface EditorContext {
  playhead: number | null;
  selected: { ref: string; start: number; end: number } | null;
}

export type MediaKind = "video" | "image" | "audio" | "captions";

export interface MediaItem {
  /** Where the file is inside the project. */
  path: string;
  name: string;
  kind: MediaKind;
  bytes: number;
  /** Length in seconds, for sound and video that could be measured. */
  duration: number | null;
}

export type ExportFormat = "mp4" | "mov" | "webm" | "gif";
export type ExportQuality = "draft" | "standard" | "delivery";

export interface ExportOptions {
  format: ExportFormat;
  quality: ExportQuality;
  /** Frames per second; the composition's own rate when null. */
  fps: number | null;
  /** A larger size of the same shape, such as "portrait-4k"; null for the composition's size. */
  resolution: string | null;
  /** The file's name, without its extension. */
  name: string;
  /** Where to save; the project's renders folder when null. */
  folder: string | null;
}

/** A problem the engine found in the composition. */
export interface CheckFinding {
  area: string;
  message: string;
  hint: string | null;
  /** The moment in the film it was seen, in seconds. */
  at: number | null;
}

export interface CheckSummary {
  ok: boolean;
  errors: CheckFinding[];
  /** Errors beyond the ones listed. */
  more_errors: number;
  warnings: number;
}

export type MusicStyle = "calm" | "steady" | "bright" | "cinematic";

export type JobSpec =
  | { type: "voiceover"; text: string; voice: string }
  | { type: "transcribe"; source: string }
  | { type: "removeBackground"; source: string }
  | ({ type: "export" } & ExportOptions)
  | { type: "music"; style: MusicStyle; seconds: number; impactAt: number | null };

export type JobStatus = "queued" | "running" | "done" | "failed" | "cancelled";

export interface Job {
  id: number;
  projectPath: string;
  kind: "voiceover" | "music" | "transcribe" | "removeBackground" | "export";
  label: string;
  status: JobStatus;
  /** What the engine last said it was doing. */
  progress: string | null;
  /** How far along it is, 0 to 100, when the engine says. */
  percent: number | null;
  /** The file the job made: relative to the project when it is inside it. */
  output: string | null;
  /** The same file's full path. */
  file: string | null;
  /** Something worth knowing about the file that was made. */
  note: string | null;
  error: string | null;
}

export interface Voice {
  id: string;
  label: string;
  language: string;
  gender: string;
}

export type ErrorCode =
  | "invalid_input"
  | "not_found"
  | "not_a_project"
  | "already_exists"
  | "refused"
  | "tool_missing"
  | "tool_failed"
  | "timeout"
  | "io"
  | "unknown";

/** The app's preferences (settings.rs). Empty model or effort leaves Claude Code's own. */
export interface Settings {
  assistantModel: "" | "fable" | "opus" | "sonnet";
  assistantEffort: "" | "low" | "medium" | "high" | "xhigh" | "max";
  instagramSearch: boolean;
  cookieBrowser: "chrome" | "safari" | "firefox" | "brave" | "edge";
  referenceMaxMinutes: number;
  projectsFolder: string;
}

// --- references (reference/model.rs) ---------------------------------------

export type ReferenceStepState = "pending" | "running" | "done" | "failed" | "skipped";

export interface ReferenceStep {
  name: "download" | "shots" | "keyframes" | "transcribe" | "analyze";
  state: ReferenceStepState;
  seconds: number | null;
  error: string | null;
}

export interface Reference {
  id: string;
  /** The link it came from, or the file's name. */
  source: string;
  platform: "youtube" | "instagram" | "tiktok" | "x" | "web" | "file";
  title: string | null;
  creator: string | null;
  duration: number | null;
  width: number | null;
  height: number | null;
  addedAt: number;
  steps: ReferenceStep[];
  status: "queued" | "running" | "ready" | "failed";
  /** Full path of the first shot's still. */
  poster: string | null;
  folder: string;
}

/** The analyst's fields keep the snake_case of the schema it answers in. */
export interface BlueprintShot {
  index: number;
  shot_type: string;
  camera_motion: string;
  subject_pose: string;
  subject_action: string;
  on_screen_text: string | null;
  product_visible: boolean;
  emotion: string;
  energy: string;
  start: number;
  end: number;
  duration: number;
  keyframe: string | null;
}

export interface Blueprint {
  referenceId: string;
  source: string;
  language: string | null;
  model: string;
  createdAt: number;
  summary: string;
  hook: { window_seconds: number; hook_type: string; analysis: string; spoken: string | null; visual: string };
  structure: { role: string; start: number; end: number; summary: string }[];
  pacing: {
    totalDuration: number;
    shotCount: number;
    avgShotLength: number;
    minShotLength: number;
    maxShotLength: number;
    cutsPer10s: number;
  };
  music: { mood: string; tempo: string; notes: string | null };
  captionStyle: { present: boolean; position: string; style: string; animation: string | null };
  shots: BlueprintShot[];
}

export interface ReferenceDetail {
  reference: Reference;
  blueprint: Blueprint | null;
}

/** A search result, not downloaded until it is picked. */
export interface FoundVideo {
  id: string;
  url: string;
  title: string;
  creator: string | null;
  duration: number | null;
  views: number | null;
  platform: "youtube" | "instagram";
  /** Full path of a cached thumbnail. */
  thumbnail: string | null;
}

// --- what the timeline draws inside clips (media_cache.rs) -----------------

export interface MediaPeaks {
  /** Seconds of sound each value covers. */
  step: number;
  /** Loudness of each step, 0 to 255. */
  values: number[];
}

export interface MediaStill {
  time: number;
  path: string;
}

// --- the engine's catalogue (catalog.rs) -----------------------------------

export type CatalogShelf = "transition" | "captions" | "texture" | "title-card" | "overlay";

export interface CatalogItem {
  name: string;
  title: string;
  description: string;
  tags: string[];
  duration: number | null;
}

export interface CatalogInstalled {
  name: string;
  snippet: string;
  files: string[];
}

// --- looks and brand kits (looks.rs, brandkit.rs) --------------------------

export interface Look {
  id: string;
  label: string;
}

export interface BrandColour {
  /** What it is for: "Background", "Accent", "Text". */
  name: string;
  hex: string;
}

export interface BrandKit {
  /** Empty for a kit not saved yet. */
  id: string;
  name: string;
  colours: BrandColour[];
  headingFont: string;
  bodyFont: string;
  /** Full path of the logo in the kit's folder. */
  logo: string | null;
  tone: string;
  website: string;
  updatedAt: number;
}

/** A review note pinned to a moment and a spot on the frame (comments.rs). */
export interface Comment {
  id: string;
  time: number;
  /** Fractions of the frame's width and height, when pinned to a spot. */
  x: number | null;
  y: number | null;
  text: string;
  resolved: boolean;
  createdAt: number;
}

/** A free sound from Openverse (openverse.rs). */
export interface FreeSound {
  id: string;
  title: string;
  creator: string | null;
  license: string;
  duration: number | null;
  url: string;
  page: string | null;
  attribution: string;
  filetype: string | null;
}
