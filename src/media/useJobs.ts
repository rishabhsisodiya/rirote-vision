import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { Job, JobSpec } from "../lib/types";

/**
 * The project's media jobs. `onFinished` runs when a job ends having made a
 * file, so whoever lists the files can list them again.
 */
export function useJobs(projectPath: string, onFinished: (job: Job) => void) {
  const [jobs, setJobs] = useState<Job[]>([]);
  const finished = useRef(onFinished);
  finished.current = onFinished;
  /** Jobs already reported as done, so each is reported once. */
  const reported = useRef(new Set<number>());

  useEffect(() => {
    let current = true;
    const show = (all: Job[]) => {
      const mine = all.filter((job) => job.projectPath === projectPath);
      setJobs(mine);
      for (const job of mine) {
        if (job.status === "done" && !reported.current.has(job.id)) {
          reported.current.add(job.id);
          finished.current(job);
        }
      }
    };
    api
      .listJobs(projectPath)
      .then((list) => {
        // What was done before this view opened is not news.
        for (const job of list) if (job.status === "done") reported.current.add(job.id);
        if (current) show(list);
      })
      .catch(() => {});
    const events = listen<Job[]>("jobs", ({ payload }) => current && show(payload));
    return () => {
      current = false;
      void events.then((unlisten) => unlisten());
    };
  }, [projectPath]);

  /** Queues a job. Resolves to why it was refused, or null when it was accepted. */
  const start = useCallback(
    async (spec: JobSpec): Promise<string | null> => {
      try {
        await api.startJob(projectPath, spec);
        return null;
      } catch (cause) {
        return errorMessage(cause);
      }
    },
    [projectPath],
  );

  const cancel = useCallback((id: number) => void api.cancelJob(id).catch(() => {}), []);
  const reveal = useCallback(
    (id: number) => void api.revealJob(projectPath, id).catch(() => {}),
    [projectPath],
  );
  const clearFinished = useCallback(
    () => void api.clearFinishedJobs(projectPath).catch(() => {}),
    [projectPath],
  );

  return { jobs, start, cancel, reveal, clearFinished };
}

export type JobControls = ReturnType<typeof useJobs>;
