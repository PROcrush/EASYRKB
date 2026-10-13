import { describe, expect, it, vi } from "vitest";

import { DEFAULT_PREFERENCES } from "./preferences";
import { answer, registerDeck, setPlaying, whenLoaded, withSetting, type DeckControls } from "./scripting";

function deck(overrides: Partial<DeckControls> = {}): DeckControls {
  return {
    track: () => "7",
    idle: () => false,
    playing: () => false,
    error: () => null,
    togglePlay: vi.fn(),
    ...overrides,
  };
}

describe("answer", () => {
  it("replies with the handler's value, or null for none", async () => {
    const handlers = { echo: (args: Record<string, unknown>) => args.say, quiet: () => undefined };
    expect(await answer(handlers, { id: 1, action: "echo", args: { say: "hi" } })).toEqual({ value: "hi" });
    expect(await answer(handlers, { id: 2, action: "quiet", args: {} })).toEqual({ value: null });
  });

  it("replies with what went wrong rather than throwing", async () => {
    const handlers = { fails: () => Promise.reject(new Error("rekordbox is running.")) };
    expect(await answer(handlers, { id: 1, action: "fails", args: {} })).toEqual({ error: "rekordbox is running." });
    expect((await answer(handlers, { id: 2, action: "toString", args: {} })).error).toMatch(/cannot/);
  });
});

describe("setPlaying", () => {
  it("presses PLAY only when the deck is not already doing what was asked", () => {
    const togglePlay = vi.fn();
    const stop = registerDeck("a", deck({ togglePlay }));
    setPlaying("a", false);
    expect(togglePlay).not.toHaveBeenCalled();
    setPlaying("a", true);
    expect(togglePlay).toHaveBeenCalledTimes(1);
    stop();
  });

  it("refuses a deck with nothing on it, and one that is not on screen", () => {
    const stop = registerDeck("a", deck({ idle: () => true }));
    expect(() => setPlaying("a", true)).toThrow("Deck 1 has no track loaded.");
    stop();
    expect(() => setPlaying("b", true)).toThrow("Deck 2 has no track loaded.");
  });
});

describe("whenLoaded", () => {
  it("waits for the deck to hold the track and be able to play it", async () => {
    let idle = true;
    const stop = registerDeck("a", deck({ idle: () => idle }));
    const loaded = whenLoaded("a", "7");
    setTimeout(() => { idle = false; }, 60);
    await expect(loaded).resolves.toBeUndefined();
    stop();
  });

  it("says why the deck could not open the track", async () => {
    const stop = registerDeck("a", deck({ idle: () => true, error: () => "The file is missing." }));
    await expect(whenLoaded("a", "7")).rejects.toThrow("The file is missing.");
    stop();
  });
});

describe("withSetting", () => {
  it("changes one choice and keeps the rest", () => {
    const next = withSetting(DEFAULT_PREFERENCES, "advanced.protectLibrary", false);
    expect(next.advanced.protectLibrary).toBe(false);
    expect(next.view).toEqual(DEFAULT_PREFERENCES.view);
  });

  it("refuses a value the choice would not keep", () => {
    expect(() => withSetting(DEFAULT_PREFERENCES, "view.keyDisplay", "sideways")).toThrow(/cannot be set/);
    expect(() => withSetting(DEFAULT_PREFERENCES, "audio.sampleRate", 12_345)).toThrow(/cannot be set/);
    expect(() => withSetting(DEFAULT_PREFERENCES, "advanced.protectLibrary", "no")).toThrow(/cannot be set/);
  });

  it("refuses a name that is not a setting", () => {
    expect(() => withSetting(DEFAULT_PREFERENCES, "view", true)).toThrow(/no setting/);
    expect(() => withSetting(DEFAULT_PREFERENCES, "view.nothing", true)).toThrow(/no setting/);
    expect(() => withSetting(DEFAULT_PREFERENCES, "keyboard.overrides", {})).toThrow(/no setting/);
    expect(() => withSetting(DEFAULT_PREFERENCES, "__proto__.x", 1)).toThrow(/no setting/);
  });
});
