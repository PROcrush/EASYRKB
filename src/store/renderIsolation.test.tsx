// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { emptyVu } from "@/lib/vuMeter";
import { __setBackend } from "@/ipc/client";
import type { Backend, BackupProgress } from "@/ipc/types";
import type { Master } from "./useMaster";
import { MasterOutputConnection, MasterOutputProvider, useMasterControls, useMasterDisplay } from "./MasterOutput";
import { useBackupProgress } from "./useBackupProgress";
import { useEventCallback } from "./useEventCallback";
import { TimeReadouts } from "@/views/player/TimeReadouts";

let master: Master;
vi.mock("./useMaster", () => ({ useMaster: () => master }));
let root: Root;
let host: HTMLDivElement;
beforeEach(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  host = document.createElement("div");
  root = createRoot(host);
  master = { level: 1, peakLeft: 0, peakRight: 0, reduction: 0, vu: emptyVu("normal"), setLevel: vi.fn() };
});
afterEach(() => { act(() => root.unmount()); vi.useRealTimers(); __setBackend(null); });

it("isolates meter updates from level consumers while keeping controls live", () => {
  let renders = 0;
  let controls: ReturnType<typeof useMasterControls>;
  function Controls() { renders++; controls = useMasterControls(); return null; }
  function Display() { return <span>{useMasterDisplay().peakLeft}</span>; }
  const children = <><Controls /><Display /></>;
  const render = () => act(() => root.render(<MasterOutputProvider><MasterOutputConnection mode="normal" />{children}</MasterOutputProvider>));
  render();
  const before = renders;
  master = { ...master, peakLeft: 0.75 };
  render();
  expect(host.textContent).toBe("0.75");
  expect(renders).toBe(before);
  master = { ...master, level: 0.5 };
  render();
  expect(controls!.level).toBe(0.5);
  controls!.setLevel(0.8);
  expect(master.setLevel).toHaveBeenCalledWith(0.8);
});

it("ignores identical backup polls but discovers jobs from another window", async () => {
  vi.useFakeTimers();
  let progress: BackupProgress = { running: false, phase: "", copiedBytes: 0, totalBytes: 0, error: null, path: null };
  __setBackend({ backupProgress: () => Promise.resolve({ ...progress }) } as Backend);
  let renders = 0;
  function Probe() { renders++; return <span>{useBackupProgress().text}</span>; }
  await act(async () => { root.render(<Probe />); await Promise.resolve(); });
  const before = renders;
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  expect(renders).toBe(before);
  progress = { ...progress, running: true, phase: "copying", totalBytes: 100, copiedBytes: 25, currentItem: "one" };
  await act(async () => { await vi.advanceTimersByTimeAsync(500); });
  expect(host.textContent).toContain("25%");
  expect(host.textContent).toContain("one");
  progress = { ...progress, currentItem: "two" };
  await act(async () => { await vi.advanceTimersByTimeAsync(500); });
  expect(host.textContent).toContain("two");
});

it("keeps event identity stable and reads the latest committed values", () => {
  let callback: () => number;
  function Probe({ value }: { value: number }) { callback = useEventCallback(() => value); return null; }
  act(() => root.render(<Probe value={1} />));
  const first = callback!;
  act(() => root.render(<Probe value={2} />));
  expect(callback!).toBe(first);
  expect(first()).toBe(2);
});

it("updates time labels on seek without rendering the parent or losing decimal tenths", () => {
  const listeners = new Set<(seconds: number) => void>();
  const source = { positionRef: { current: 0 }, subscribe: (fn: (seconds: number) => void) => { listeners.add(fn); return () => { listeners.delete(fn); }; } };
  let renders = 0;
  function Probe() { renders++; return <TimeReadouts source={source} total={300} classes={{}} />; }
  act(() => root.render(<Probe />));
  for (const value of [1.2, 61.9, 0, 300]) {
    act(() => { source.positionRef.current = value; for (const fn of listeners) fn(value); });
    const tenths = Math.floor(value * 10);
    expect(host.lastElementChild?.textContent).toBe(`${String(Math.floor(value / 60)).padStart(2, "0")}:${String(Math.floor(value) % 60).padStart(2, "0")}.${tenths % 10}`);
  }
  expect(renders).toBe(1);
  act(() => root.unmount());
  expect(listeners.size).toBe(0);
});
