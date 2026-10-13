/**
 * Tests updater Markdown, Discord payload formatting, and rejection of a
 * missing release version with in-memory release-note fixtures.
 * Run: node --test scripts/release-notes.test.mjs.
 */
import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { discordPayload, releaseForVersion, updaterMarkdown } from "./release-notes.mjs";

const notes = [{
  version: "1.0.0-rc.1",
  date: "2026-09-24",
  changes: [
    "(New) A new thing.",
    "(Fixed) A broken thing works.",
    "(Improved) A slow thing is faster.",
  ],
}];

describe("release notes", () => {
  it("renders the updater markdown from the structured notes", () => {
    const markdown = updaterMarkdown(notes);
    assert.match(markdown, /^## \[1\.0\.0-rc\.1\] — 2026-09-24/m);
    assert.match(markdown, /### Added\n- A new thing\./);
    assert.match(markdown, /### Fixed\n- A broken thing works\./);
    assert.match(markdown, /### Improved\n- A slow thing is faster\./);
  });

  it("builds a Discord announcement linked to the matching website entry", () => {
    const payload = discordPayload(notes[0], "https://example.com/whats-new/");
    assert.equal(payload.embeds[0].title, "rbxport 1.0.0-rc.1 released");
    assert.equal(payload.embeds[0].url, "https://example.com/whats-new/#v1.0.0-rc.1");
    assert.match(payload.embeds[0].description, /### Fixed/);
  });

  it("refuses a version that has no release-note entry", () => {
    assert.throws(() => releaseForVersion(notes, "1.0.0"), /no entry/);
  });
});
