import { useEffect, useState } from "react";
import { getBackend } from "@/ipc/client";
import type { ExportProgress } from "@/ipc/types";

export function exportPercent(progress: ExportProgress): number {
  return progress.state === "done" ? 100 : progress.total > 0
    ? Math.min(99, Math.max(0, Math.floor(progress.done / progress.total * 100))) : 0;
}

/** All windows follow the backend job, independently of the initiating view. */
export function useExportProgress() {
  const [jobs, setJobs] = useState<ReadonlyMap<string, ExportProgress>>(new Map());
  useEffect(() => {
    let live = true;
    let batchStarted = false;
    let stop: (() => void) | undefined;
    void getBackend().then(backend => {
      if (!live) return;
      const updated = new Set<string>();
      stop = backend.onExportProgress(progress => {
        updated.add(progress.path);
        if (progress.state === "preparing") batchStarted = true;
        if (live) setJobs(current => {
          const active = [...current.values()].some(job => ["preparing", "checking", "copying", "database", "verifying", "publishing", "ejecting"].includes(job.state));
          const next = progress.state === "preparing" && !active ? new Map<string, ExportProgress>() : new Map(current);
          return next.set(progress.path, progress);
        });
      });
      void backend.exportProgress().then(jobs => {
        if (live) setJobs(current => {
          // A preparing event is newer than this startup snapshot. Merging
          // the snapshot would resurrect terminal jobs from the prior batch
          // and make one selected stick read as two in the aggregate meter.
          if (batchStarted) return current;
          const next = new Map(current);
          for (const job of jobs) if (!updated.has(job.path)) next.set(job.path, job);
          return next;
        });
      }).catch(() => {});
    });
    return () => { live = false; stop?.(); };
  }, []);
  return jobs;
}
