/** @vitest-environment jsdom */
import { act, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { __setBackend } from "@/ipc/client";
import { createMockBackend } from "@/ipc/backend-mock";
import { zoomBy } from "@/lib/player";
import type { PlayerProps } from "@/views/player/Player";

// Keep the real shell and shared controls. The deck stand-in exposes the
// controlled zoom and the same onBars/publishZoom seam used by its wheel.
vi.mock("@/views/player/Player", () => ({
  Player: ({ deck = "a", bars = 12, onBars, publishZoom, onMaster }: PlayerProps) => {
    useEffect(() => publishZoom?.((by) => onBars?.(zoomBy(bars, by))), [publishZoom, onBars, bars]);
    return <div data-deck={deck} data-bars={bars}>
      <button onClick={() => onBars?.(zoomBy(bars, -1))}>Wheel {deck}</button>
      <button onClick={onMaster}>Master {deck}</button>
    </div>;
  },
}));
vi.mock("@/views/browser/TrackTable", () => ({ TrackTable: () => null }));
vi.mock("@/views/tree/TreeView", () => ({ TreeView: () => null }));
vi.mock("@/views/topbar/TopBar", () => ({
  ConnectedTopBar: ({ onLayoutChange }: { onLayoutChange: (layout: "one" | "two") => void }) => <>
    <button onClick={() => onLayoutChange("one")}>Layout one</button>
    <button onClick={() => onLayoutChange("two")}>Layout two</button>
  </>,
}));
vi.mock("@/views/statusbar/StatusBar", () => ({ StatusBar: () => null }));
vi.mock("@/views/browser/RightRail", () => ({ RightRail: () => null }));
vi.mock("@/views/player/MixerStrip", () => ({ MixerStrip: () => null }));
vi.mock("@/views/topbar/AppCost", () => ({ AppCost: () => null }));

import { App } from "./App";

let host: HTMLDivElement;
let root: Root;
beforeEach(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  localStorage.clear();
  __setBackend(createMockBackend({ trackCount: 2 }));
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
  __setBackend(null);
  vi.restoreAllMocks();
  localStorage.clear();
});
async function render(dualControl = false) {
  localStorage.setItem("rbl.session", JSON.stringify({ layout: "two", waveformZoom: { a: 12, b: 4 }, dualControl }));
  await act(async () => { root.render(<App />); await Promise.resolve(); });
}
const bars = () => ["a", "b"].map(deck => Number(host.querySelector(`[data-deck="${deck}"]`)?.getAttribute("data-bars")));
function click(label: string) {
  const button = [...host.querySelectorAll("button")].find(b => (b.getAttribute("aria-label") ?? b.textContent) === label);
  expect(button, label).toBeDefined();
  act(() => button!.click());
}

it.each([false, true])("restores one zoom from an old session with dual control %s", async (dualControl) => {
  await render(dualControl);
  expect(bars()).toEqual([12, 12]);
});
it("shared zoom buttons keep both decks at the same level", async () => {
  await render();
  click("Zoom in");
  expect(bars()).toEqual([8, 8]);
  click("Zoom out");
  expect(bars()).toEqual([12, 12]);
});
it("a wheel on either deck zooms both with dual control off", async () => {
  await render();
  click("Wheel a");
  expect(bars()).toEqual([8, 8]);
  click("Wheel b");
  expect(bars()).toEqual([4, 4]);
});
it("switching dual control on and off preserves synchronized wheel zoom", async () => {
  await render();
  click("Wheel a");
  click("Dual control");
  expect(bars()).toEqual([8, 8]);
  click("Wheel b");
  expect(bars()).toEqual([4, 4]);
  click("Dual control");
  expect(bars()).toEqual([4, 4]);
  click("Wheel a");
  expect(bars()).toEqual([2, 2]);
});
it("switching the master does not change the shared zoom", async () => {
  await render();
  click("Wheel a");
  click("Master b");
  expect(bars()).toEqual([8, 8]);
  click("Zoom out");
  expect(bars()).toEqual([12, 12]);
  click("Wheel b");
  expect(bars()).toEqual([8, 8]);
});
it("keeps the current zoom when switching between one and two players", async () => {
  await render();
  click("Wheel b");
  click("Layout one");
  click("Wheel a");
  click("Layout two");
  expect(bars()).toEqual([4, 4]);
});
