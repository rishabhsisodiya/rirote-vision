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
  listVoices: () => call<Voice[]>("voices_list"),
  listJobs: (path: string) => call<Job[]>("jobs_list", { path }),
  startJob: (path: string, spec: JobSpec) => call<Job>("job_start", { path, spec }),
  cancelJob: (id: number) => call<void>("job_cancel", { id }),
  revealJob: (path: string, id: number) => call<void>("job_reveal", { path, id }),
  checkExport: (path: string) => call<CheckSummary>("export_check", { path }),
  clearFinishedJobs: (path: string) => call<void>("jobs_clear_finished", { path }),
  launchProject: () => call<string | null>("launch_project"),
  launchAutoplay: () => call<boolean>("launch_autoplay"),
};

/** Records a line in the app's log. Never throws: a log is not worth a failure. */
export function logToApp(message: string): void {
  invoke("client_log", { message }).catch(() => {});
}
