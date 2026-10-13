import { describe, expect, it } from "vitest";

import { exportSummary } from "./exportSummary";

const base = { tracks: 0, reused: 0, removed: 0, playlistsAdded: 0, playlistsRemoved: 0, skipped: [] as string[], verified: true };

describe("exportSummary", () => {
  it("reports a first export by its size", () => {
    expect(exportSummary("Set", { ...base, tracks: 12 })).toBe(
      "Set: Updated 12 tracks. Skipped 0 tracks (no change). Added 0 playlists. Removed 0 playlists.",
    );
  });

  it("reports a second export by what changed", () => {
    expect(exportSummary("Set", { ...base, tracks: 12, reused: 10, removed: 3 })).toBe(
      "Set: Updated 2 tracks. Skipped 10 tracks (no change). Added 0 playlists. Removed 0 playlists.",
    );
  });

  it("leaves out the parts that are zero", () => {
    expect(exportSummary("Set", { ...base, tracks: 10, reused: 10 })).toBe(
      "Set: Updated 0 tracks. Skipped 10 tracks (no change). Added 0 playlists. Removed 0 playlists.",
    );
  });

  it("counts one track as a track", () => {
    expect(exportSummary("Set", { ...base, tracks: 1 })).toContain("Updated 1 track");
    expect(exportSummary("Set", { ...base, tracks: 2, reused: 1, removed: 1 })).toContain("Updated 1 track");
  });

  it("says when tracks were left behind", () => {
    expect(exportSummary("Set", { ...base, tracks: 3, skipped: ["Gone"] })).toContain(
      "1 track missing",
    );
  });

  it("says plainly when the stick did not read back", () => {
    expect(exportSummary("Set", { ...base, tracks: 3, verified: false })).toContain(
      "did not read back",
    );
  });
});
