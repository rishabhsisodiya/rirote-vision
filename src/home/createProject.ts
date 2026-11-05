import { api } from "../lib/ipc";
import type { ProjectSummary, Resolution } from "../lib/types";
import { nextName } from "./briefs";

/** Creates a project in the projects folder, numbering the name if it is taken. */
export async function createNamed(name: string, resolution: Resolution): Promise<ProjectSummary> {
  const parent = await api.projectsFolder();
  let attempt = name;
  for (let tries = 0; tries < 20; tries += 1) {
    try {
      return await api.createProject({ parent, name: attempt, resolution });
    } catch (cause) {
      if (cause instanceof Error && "code" in cause && cause.code === "already_exists") {
        attempt = nextName(attempt);
        continue;
      }
      throw cause;
    }
  }
  throw new Error(`Too many projects are already called “${name}”. Rename one and try again.`);
}
