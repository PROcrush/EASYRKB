/**
 * Tests release-note filtering, reviewed trailers, deduplication, and
 * explicit ticket attribution using in-memory commit fixtures.
 * Run: node --test scripts/generate-release-notes.test.mjs.
 */
import assert from "node:assert/strict";
import test from "node:test";
import { changesFromCommits, generateReleaseNotes } from "./generate-release-notes.mjs";

test("exclude maintenance and test-only commits, even when titled fix", () => {
  assert.deepEqual(changesFromCommits([
    { subject: "fix: continue releases after skipped tests", files: [".github/workflows/release.yml"] },
    { subject: "fix: satisfy backup manifest assertions", files: ["crates/rbl-backup/src/manifest.rs"] },
    { subject: "fix: check playback", files: ["crates/rbl-deck/tests/engine.rs"] },
  ]), []);
});

test("reviewed trailers group descriptions and control ticket attribution", () => {
  const note = "(Fixed) RBX-25: Playlist positions stay correct when sorting.";
  assert.deepEqual(changesFromCommits([
    { subject: "fix: sorting", body: `Release-Note: ${note}` },
    { subject: "fix: sorting followup", body: `Release-Note: ${note}` },
    { subject: "fix: repair artwork", body: "Related discussion RBX-25", files: ["src/views/Info.tsx"] },
  ]), [note, "(Fixed) Repair artwork."]);
});

test("refuses malformed curated notes before writing release metadata", () => {
  assert.throws(
    () => generateReleaseNotes("9.9.9", "HEAD", "HEAD", JSON.stringify({ changes: ["Internal refactor"] })),
    /Invalid curated release note/,
  );
});
