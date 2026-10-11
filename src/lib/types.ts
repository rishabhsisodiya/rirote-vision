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
export type SetupAction = "install-engine" | "install-voice" | "install-skills";

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
  | { type: "setVolume"; clip: string; volume: number };

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
