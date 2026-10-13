import type { AnalysisChoice } from "@/views/analysis/AnalysisDialog";

const KEY = "rbxport.analysisChoice";

/** What the Analysis Setting window was last confirmed with, if anything. */
export function lastAnalysisChoice(): Partial<AnalysisChoice> {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (typeof raw !== "object" || raw === null) return {};
    const saved = raw as Record<string, unknown>;
    const out: Partial<AnalysisChoice> = {};
    for (const flag of ["bpmGrid", "key", "highPrecision", "firstBeatCue"] as const) {
      if (typeof saved[flag] === "boolean") out[flag] = saved[flag];
    }
    if (saved.mode === "rekordbox" || saved.mode === "rbxport") out.mode = saved.mode;
    if (typeof saved.minBpm === "number" && typeof saved.maxBpm === "number") {
      out.minBpm = saved.minBpm;
      out.maxBpm = saved.maxBpm;
    }
    return out;
  } catch {
    return {};
  }
}

export function rememberAnalysisChoice(choice: AnalysisChoice): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(choice));
  } catch {
    // Not remembering is only a convenience lost.
  }
}
