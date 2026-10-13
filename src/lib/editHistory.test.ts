/** @vitest-environment jsdom */
import { afterEach, describe, expect, it, vi } from "vitest";
import { listenEditHistory, runEditHistory, setLibraryEditHistory } from "./editHistory";
import { setHistoryMenu } from "@/ipc/client";

vi.mock("@/ipc/client", () => ({ setHistoryMenu: vi.fn().mockResolvedValue(undefined) }));

afterEach(() => {
  setLibraryEditHistory(null, null);
  document.body.replaceChildren();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe("native Edit history routing", () => {
  it("shows the action at the top of the library history", () => {
    setLibraryEditHistory("Track Edit", "Delete Playlist");
    expect(setHistoryMenu).toHaveBeenLastCalledWith("Track Edit", "Delete Playlist");
  });

  it("falls back to library history for actions the deck cannot run", () => {
    setLibraryEditHistory("Delete Playlist", null);
    const stop = listenEditHistory(vi.fn(), null, "Set Tempo");
    try {
      expect(setHistoryMenu).toHaveBeenLastCalledWith("Delete Playlist", "Set Tempo");
    } finally {
      stop();
    }
  });

  it("describes the active history, resets for text focus and clears on deactivation", () => {
    const field = document.createElement("input");
    document.body.append(field);
    const stop = listenEditHistory(vi.fn(), "Shift Beat Grid Left", "Set Tempo");
    try {
      expect(setHistoryMenu).toHaveBeenLastCalledWith("Shift Beat Grid Left", "Set Tempo");
      field.focus();
      expect(setHistoryMenu).toHaveBeenLastCalledWith(null, null);
      field.blur();
      expect(setHistoryMenu).toHaveBeenLastCalledWith("Shift Beat Grid Left", "Set Tempo");
    } finally {
      stop();
    }
    expect(setHistoryMenu).toHaveBeenLastCalledWith(null, null);
  });
  it("delivers undo and redo to the subscribed deck, and cleans up on deactivation", () => {
    const deck = vi.fn();
    const stop = listenEditHistory(deck);
    try {
      runEditHistory("undo");
      runEditHistory("redo");
      expect(deck.mock.calls).toEqual([["undo"], ["redo"]]);
    } finally {
      stop();
    }
    runEditHistory("undo");
    expect(deck).toHaveBeenCalledTimes(2);
  });

  it.each(["input", "textarea"])("keeps menu history in a focused %s", (tag) => {
    const field = document.createElement(tag);
    document.body.append(field);
    field.focus();
    const textHistory = vi.fn();
    const original = Object.getOwnPropertyDescriptor(document, "execCommand");
    Object.defineProperty(document, "execCommand", { configurable: true, value: textHistory });
    const deck = vi.fn();
    const stop = listenEditHistory(deck);
    try {
      runEditHistory("undo");
      runEditHistory("redo");
      expect(textHistory.mock.calls).toEqual([["undo"], ["redo"]]);
      expect(deck).not.toHaveBeenCalled();
    } finally {
      stop();
      if (original) Object.defineProperty(document, "execCommand", original);
      else Reflect.deleteProperty(document, "execCommand");
    }
  });
});
