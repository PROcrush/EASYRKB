/** @vitest-environment jsdom */
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { __setBackend } from "@/ipc/client";
import { createMockBackend } from "@/ipc/backend-mock";
import type { ExportReport, TreeNode } from "@/ipc/types";
import { DEFAULT_PREFERENCES, loadPreferences, savePreferences } from "@/lib/preferences";

// Exercise the real App export callbacks and confirmation dialog. Only the
// initiating views and backend are replaced; no mounted USB is written.
vi.mock("@/views/browser/TrackTable", () => ({ TrackTable: ({ onExportTrack }: {
  onExportTrack?: (path: string, ids: readonly string[]) => void;
}) => <button onClick={() => onExportTrack?.("/Volumes/Fixture", ["1"])}>Export track fixture</button> }));
vi.mock("@/views/tree/TreeView", () => ({ TreeView: ({ onExport }: {
  onExport?: (node: TreeNode, path: string) => void;
}) => <button onClick={() => onExport?.({ id: "p1", name: "Fixture", kind: "playlist", depth: 0 }, "/Volumes/Fixture")}>Export playlist fixture</button> }));
vi.mock("@/views/player/Player", () => ({ Player: () => null }));
vi.mock("@/views/topbar/TopBar", () => ({ ConnectedTopBar: () => null }));
vi.mock("@/views/statusbar/StatusBar", () => ({ StatusBar: () => null }));
vi.mock("@/views/browser/RightRail", () => ({ RightRail: () => null }));
vi.mock("@/views/player/MixerStrip", () => ({ MixerStrip: () => null }));
vi.mock("@/views/topbar/AppCost", () => ({ AppCost: () => null }));

import { App } from "./App";

const REPORT: ExportReport = { tracks: 1, playlists: 1, bytesCopied: 100, analysisFiles: 1,
  reused: 0, removed: 0, tracksAdded: 1, tracksUpdated: 0, playlistsAdded: 1, playlistsRemoved: 0,
  skipped: [], failed: [], verified: true };
let host: HTMLDivElement;
let root: Root;
const exportTrack = vi.fn().mockResolvedValue(REPORT);
const exportPlaylist = vi.fn().mockResolvedValue(REPORT);
beforeEach(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  localStorage.clear();
  HTMLDialogElement.prototype.showModal = vi.fn();
  HTMLDialogElement.prototype.close = vi.fn();
  exportTrack.mockClear();
  exportPlaylist.mockClear();
  const backend = createMockBackend({ trackCount: 2, writable: true });
  backend.listDevices = () => Promise.resolve([{ name: "Fixture", path: "/Volumes/Fixture", totalBytes: 1_000,
    freeBytes: 900, fileSystem: "FAT32", removable: true, volumeId: "fixture", export: null }]);
  backend.exportTracksToDevice = exportTrack;
  backend.exportPlaylist = exportPlaylist;
  __setBackend(backend);
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
  __setBackend(null);
  localStorage.clear();
});
async function render(maximumCompatibility = false) {
  savePreferences({ ...DEFAULT_PREFERENCES, usbExport: { ...DEFAULT_PREFERENCES.usbExport, maximumCompatibility, conversionFormat: "mp3" } });
  await act(async () => { root.render(<App />); await Promise.resolve(); });
}
async function click(label: string) {
  const button = [...host.querySelectorAll("button")].find(b => b.textContent === label);
  expect(button, label).toBeDefined();
  await act(async () => { button!.click(); await Promise.resolve(); });
}
function selectAudio(value: "original" | "convert", format = "wav") {
  act(() => host.querySelector<HTMLInputElement>(`input[name="export-audio-conversion"][value="${value}"]`)!.click());
  if (value === "convert") {
    const select = host.querySelector<HTMLSelectElement>("#export-conversion-format")!;
    act(() => { select.value = format; select.dispatchEvent(new Event("change", { bubbles: true })); });
  }
}
it.each(["track", "playlist"] as const)("passes this export's conversion and library choices through the %s entry point", async kind => {
  await render();
  await click(`Export ${kind} fixture`);
  selectAudio("convert", kind === "track" ? "wav" : "aiff");
  act(() => host.querySelector<HTMLInputElement>('input[value="deviceLibrary"]')!.click());
  await click("Start export");
  const calls = (kind === "track" ? exportTrack : exportPlaylist).mock.calls;
  expect(calls).toHaveLength(1);
  expect(calls[0]![0]).toEqual(kind === "track" ? ["1"] : "p1");
  expect(calls[0]!.slice(-2)).toEqual([kind === "track" ? "wav" : "aiff", "deviceLibrary"]);
  expect(loadPreferences().usbExport.maximumCompatibility).toBe(false);
});
it.each(["track", "playlist"] as const)("can preserve originals for %s export despite a saved conversion preference", async kind => {
  await render(true);
  await click(`Export ${kind} fixture`);
  selectAudio("original");
  await click("Start export");
  const calls = (kind === "track" ? exportTrack : exportPlaylist).mock.calls;
  expect(calls).toHaveLength(1);
  expect(calls[0]!.slice(-2)).toEqual([undefined, "both"]);
  expect(loadPreferences().usbExport.maximumCompatibility).toBe(true);
});
it.each(["track", "playlist"] as const)("cancelling %s export makes no export call", async kind => {
  await render();
  await click(`Export ${kind} fixture`);
  await click("Cancel");
  expect(exportTrack).not.toHaveBeenCalled();
  expect(exportPlaylist).not.toHaveBeenCalled();
  expect(host.querySelector("dialog")).toBeNull();
});
