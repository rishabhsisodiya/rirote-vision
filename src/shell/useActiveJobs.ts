import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

import type { Job } from "../lib/types";

/** How many jobs, in any project, are queued or running. */
export function useActiveJobs(): number {
  const [count, setCount] = useState(0);
  useEffect(() => {
    const stop = listen<Job[]>("jobs", ({ payload }) => {
      setCount(payload.filter((job) => job.status === "queued" || job.status === "running").length);
    });
    return () => {
      void stop.then((unlisten) => unlisten());
    };
  }, []);
  return count;
}
