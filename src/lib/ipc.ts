// The only place the interface talks to the Rust side.

import { invoke } from "@tauri-apps/api/core";

import type {
  ApprovalAnswer,
  CheckSummary,
  ChatView,
  EditOp,
  EditorContext,
  ErrorCode,
  Job,
  JobSpec,
  MediaItem,
  NewProjectInput,
  PreviewSession,
  ProjectInfo,
  ProjectSummary,
  RecentProject,
  SetupReport,
  Reference,
  Look,
  FreeSound,
  Comment,
  BrandKit,
  MediaPeaks,
  MediaStill,
  CatalogShelf,
  CatalogItem,
  CatalogInstalled,
  ReferenceDetail,
  Settings,
  TimelineState,
  TrustLevel,
  Voice,
} from "./types";

export class AppError extends Error {
  constructor(
    readonly code: ErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "AppError";
  }
}

function toAppError(cause: unknown): AppError {
  if (cause instanceof AppError) return cause;
  if (typeof cause === "object" && cause !== null && "message" in cause) {
    const { code, message } = cause as { code?: ErrorCode; message: unknown };
    return new AppError(code ?? "unknown", String(message));
  }
  return new AppError("unknown", typeof cause === "string" ? cause : "Something went wrong.");
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw toAppError(cause);
  }
}

/** The text to show a person for any thrown value. */
export function errorMessage(cause: unknown): string {
  return toAppError(cause).message;
}

export const api = {
  environmentCheck: () => call<SetupReport>("environment_check"),
  engineInstall: () => call<SetupReport>("engine_install"),
  voiceInstall: () => call<SetupReport>("voice_install"),
  skillsInstall: () => call<SetupReport>("skills_install"),
  ytdlpInstall: () => call<SetupReport>("ytdlp_install"),
  recentProjects: () => call<RecentProject[]>("projects_recent"),
  openProject: (path: string) => call<ProjectSummary>("project_open", { path }),
  createProject: (input: NewProjectInput) => call<ProjectSummary>("project_create", { input }),
  forgetProject: (path: string) => call<void>("project_forget", { path }),
  projectInfo: (path: string) => call<ProjectInfo>("project_info", { path }),
  revealProject: (path: string) => call<void>("project_reveal", { path }),
  startPreview: (path: string, fresh: boolean) =>
    call<PreviewSession>("preview_start", { path, fresh }),
  releasePreview: (lease: number) => call<void>("preview_release", { lease }),
  readTimeline: (path: string) => call<TimelineState>("timeline_read", { path }),
  editTimeline: (path: string, op: EditOp) => call<TimelineState>("timeline_edit", { path, op }),
  undoTimeline: (path: string) => call<TimelineState>("timeline_undo", { path }),
  redoTimeline: (path: string) => call<TimelineState>("timeline_redo", { path }),
  loadChat: (path: string) => call<ChatView>("chat_load", { path }),
  sendChat: (path: string, prompt: string, context: EditorContext) =>
    call<void>("chat_send", { path, prompt, context }),
  answerChat: (requestId: string, answer: ApprovalAnswer) =>
    call<void>("chat_answer", { requestId, answer }),
  stopChat: () => call<void>("chat_stop"),
  clearChat: (path: string) => call<ChatView>("chat_clear", { path }),
  setChatTrust: (path: string, trust: TrustLevel) =>
    call<ChatView>("chat_set_trust", { path, trust }),
  forgetChatRules: (path: string) => call<ChatView>("chat_forget_rules", { path }),
  listMedia: (path: string) => call<MediaItem[]>("media_list", { path }),
  importMedia: (path: string, sources: string[]) =>
    call<MediaItem[]>("media_import", { path, sources }),
  revealMedia: (path: string, file: string) => call<void>("media_reveal", { path, file }),
  /** The composition files that use a media file. */
  mediaUsedBy: (path: string, file: string) => call<string[]>("media_used_by", { path, file }),
  /** Moves a media file to the Trash. */
  trashMedia: (path: string, file: string) => call<void>("media_trash", { path, file }),
  listVoices: () => call<Voice[]>("voices_list"),
  /** Makes (once) and returns the file of a short spoken sample of `voice`. */
  voiceSample: (voice: string) => call<string>("voice_sample", { voice }),
  listJobs: (path: string) => call<Job[]>("jobs_list", { path }),
  startJob: (path: string, spec: JobSpec) => call<Job>("job_start", { path, spec }),
  cancelJob: (id: number) => call<void>("job_cancel", { id }),
  revealJob: (path: string, id: number) => call<void>("job_reveal", { path, id }),
  checkExport: (path: string) => call<CheckSummary>("export_check", { path }),
  clearFinishedJobs: (path: string) => call<void>("jobs_clear_finished", { path }),
  launchProject: () => call<string | null>("launch_project"),
  launchAutoplay: () => call<boolean>("launch_autoplay"),
  getSettings: () => call<Settings>("settings_get"),
  projectsFolder: () => call<string>("projects_folder"),
  readTranscript: (path: string, src: string) =>
    call<{ file: string; words: { text: string; start: number; end: number }[] } | null>("transcript_read", { path, src }),
  mediaSilences: (path: string, src: string, noiseDb: number, minSeconds: number) =>
    call<[number, number][]>("media_silences", { path, src, noiseDb, minSeconds }),
  cutTimeline: (path: string, clip: string, ranges: [number, number][]) =>
    call<TimelineState>("timeline_cut", { path, clip, ranges }),
  listSkills: () => call<{ name: string; description: string }[]>("skills_list"),
  listHistory: (path: string) =>
    call<{ id: string; label: string; by: string; name: string; endedAt: number | null; files: string[]; undone: boolean }[]>("history_list", { path }),
  undoHistory: (path: string, id: string) => call<void>("history_undo", { path, id }),
  listComments: (path: string) => call<Comment[]>("comments_list", { path }),
  addComment: (path: string, time: number, x: number | null, y: number | null, text: string) =>
    call<Comment[]>("comments_add", { path, time, x, y, text }),
  updateComment: (path: string, id: string, change: { text?: string; resolved?: boolean }) =>
    call<Comment[]>("comments_update", { path, id, text: change.text ?? null, resolved: change.resolved ?? null }),
  deleteComment: (path: string, id: string) => call<Comment[]>("comments_delete", { path, id }),
  searchSounds: (query: string, commercial: boolean) => call<FreeSound[]>("sounds_search", { query, commercial }),
  downloadSound: (path: string, sound: FreeSound) => call<string>("sounds_download", { path, sound }),
  listLooks: () => call<Look[]>("looks_list"),
  applyLook: (path: string, clipId: string, preset: string | null, intensity: number) =>
    call<void>("look_apply", { path, clipId, preset, intensity }),
  listBrandKits: () => call<BrandKit[]>("brandkit_list"),
  saveBrandKit: (kit: BrandKit) => call<BrandKit>("brandkit_save", { kit }),
  setBrandLogo: (id: string, file: string) => call<BrandKit>("brandkit_logo", { id, file }),
  deleteBrandKit: (id: string) => call<void>("brandkit_delete", { id }),
  brandBrief: (id: string) => call<string>("brandkit_brief", { id }),
  projectBrand: (path: string) => call<BrandKit | null>("project_brand_get", { path }),
  setProjectBrand: (path: string, id: string | null) => call<BrandKit | null>("project_brand_set", { path, id }),
  mediaPeaks: (path: string, src: string) => call<MediaPeaks>("media_peaks", { path, src }),
  mediaStills: (path: string, src: string) => call<MediaStill[]>("media_stills", { path, src }),
  listCatalog: (shelf: CatalogShelf) => call<CatalogItem[]>("catalog_list", { shelf }),
  addFromCatalog: (path: string, name: string) => call<CatalogInstalled>("catalog_add", { path, name }),
  listReferences: () => call<Reference[]>("reference_list"),
  getReference: (id: string) => call<ReferenceDetail>("reference_get", { id }),
  importReference: (input: { link?: string; file?: string }) =>
    call<Reference>("reference_import", { link: input.link ?? null, file: input.file ?? null }),
  retryReference: (id: string) => call<Reference>("reference_retry", { id }),
  stopReference: (id: string) => call<void>("reference_stop", { id }),
  deleteReference: (id: string) => call<void>("reference_delete", { id }),
  /** `voice`: "" matches the reference, "none" is no voiceover, else a voice id. */
  referenceBrief: (id: string, request: string, voice: string) => call<string>("reference_brief", { id, request, voice }),
  projectPoster: (path: string) => call<string>("project_poster", { path }),
  setSettings: (settings: Settings) => call<Settings>("settings_set", { settings }),
};

/** Records a line in the app's log. Never throws: a log is not worth a failure. */
export function logToApp(message: string): void {
  invoke("client_log", { message }).catch(() => {});
}
