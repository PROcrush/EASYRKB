/**
 * @vitest-environment jsdom
 *
 * What the master meters do when the music stops.
 *
 * The engine's ticker ends the moment neither deck is playing, so the last
 * reading to arrive is the one taken just before the stop — a loud one. Left
 * to the readings alone the bars would stay where the music left them. These
 * mount the hook against a stub engine and drive the meter events, the timer
 * and the frames by hand.
 */
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { __setBackend } from "@/ipc/client";
import type { Backend, Meters, Tick } from "@/ipc/types";
import { FULL_GAIN } from "@/lib/volume";
import { useMaster, type Master } from "./useMaster";

declare global {
  var IS_REACT_ACT_ENVIRONMENT: boolean;
}

/** Past the wait for a reading that is not coming. */
const SILENCE_WAIT_MS = 150;

let host: HTMLDivElement;
let root: Root;
let master: Master;
/** The meter listener the hook registered. */
let metered: (meters: Meters) => void;
/** Pending animation frames, run only when a test says so. */
let frames: (() => void)[];
/** The clock the hook reads, advanced only by `elapse`. */
let clock: number;
let engineLevel = 1;
const setMasterLevel = vi.fn((level: number) => { engineLevel = level; return Promise.resolve(); });

const idle: Tick["a"] = {
  frames: 0, totalFrames: 0, generation: 0, playing: false, loaded: false,
  tempo: 1, masterTempo: false, keyShift: 0, startInFrames: 0, loopInFrames: 0, loopOutFrames: 0, looping: false,
};

function stubBackend(): Backend {
  return {
    deckState: () => Promise.resolve({
      a: idle, b: idle, sampleRate: 44_100, peakLeft: 0, peakRight: 0, master: engineLevel, reduction: 0, shiftsKey: true,
    } satisfies Tick),
    onDeckTick: () => () => {},
    onDeckEvent: () => () => {},
    onMeters: (listener: (meters: Meters) => void) => {
      metered = listener;
      return () => {};
    },
    setMasterLevel,
  } as unknown as Backend;
}

function Probe() {
  master = useMaster();
  return null;
}

beforeEach(async () => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  frames = [];
  localStorage.clear();
  engineLevel = 1;
  setMasterLevel.mockClear();
  clock = 1000;
  vi.useFakeTimers();
  vi.stubGlobal("performance", { now: () => clock });
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
    frames.push(() => cb(clock));
    return frames.length;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => {
    frames[id - 1] = () => {};
  });
  __setBackend(stubBackend());
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  await act(async () => {
    root.render(<Probe />);
    await Promise.resolve();
    await Promise.resolve();
  });
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
  __setBackend(null);
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

/** Moves both the clock the hook reads and the one the timer runs on. */
function elapse(ms: number) {
  clock += ms;
  act(() => {
    vi.advanceTimersByTime(ms);
  });
}

/** One reading, as the engine sends them thirty times a second. */
function reading(peak: number) {
  act(() => {
    metered({ peakLeft: peak, peakRight: peak, master: 1, reduction: 0 });
  });
}

/** Runs whatever frames are queued, the way the browser would. */
function runFrames(count: number) {
  for (let i = 0; i < count; i++) {
    const queued = frames;
    frames = [];
    if (queued.length === 0) return;
    clock += 1000 / 60;
    act(() => {
      for (const frame of queued) frame();
    });
  }
}

describe("the master meters when the readings stop", () => {
  it("comes down to nothing after the last reading", () => {
    elapse(33);
    reading(1);
    expect(master.peakLeft).toBe(1);

    // The engine's ticker has stopped, so no further reading is coming.
    elapse(SILENCE_WAIT_MS);
    runFrames(600);

    expect(master.peakLeft).toBe(0);
    expect(master.peakRight).toBe(0);
  });

  it("holds the bars while the readings keep coming", () => {
    // Three seconds of music, more than long enough for a stalled meter to
    // have fallen all the way if the fall were running against the readings.
    for (let i = 0; i < 90; i++) {
      elapse(33);
      reading(0.8);
    }
    expect(master.peakLeft).toBeCloseTo(0.8, 6);
    // Nothing is animating: the readings alone are driving the meter.
    expect(frames).toHaveLength(0);
  });

  it("stops drawing frames once there is nothing left to fall", () => {
    elapse(33);
    reading(1);
    elapse(SILENCE_WAIT_MS);
    runFrames(600);

    expect(master.peakLeft).toBe(0);
    // The window is idle again, so the fall must not still be asking for
    // frames — that would be a permanent cost for a bar at nothing.
    expect(frames).toHaveLength(0);
  });

  it("takes the music back up mid-fall", () => {
    elapse(33);
    reading(1);
    elapse(SILENCE_WAIT_MS);
    runFrames(10);
    const partway = master.peakLeft;
    expect(partway).toBeLessThan(1);
    expect(partway).toBeGreaterThan(0);

    // A reading arriving again is the music playing again.
    reading(0.9);
    expect(master.peakLeft).toBe(0.9);
    // And the fall has stood down rather than running on underneath it.
    runFrames(5);
    expect(master.peakLeft).toBe(0.9);
  });

  it("leaves the level alone while the bars fall", () => {
    elapse(33);
    act(() => {
      metered({ peakLeft: 1, peakRight: 1, master: 0.62, reduction: 0 });
    });
    elapse(SILENCE_WAIT_MS);
    runFrames(600);

    // The knob is not a meter: silence does not turn it down.
    expect(master.peakLeft).toBe(0);
    expect(master.level).toBe(0.62);
  });
});


it.each([0, 0.37, 1, FULL_GAIN])("restores master volume %s after a fresh session", async (level) => {
  await act(async () => { master.setLevel(level); await Promise.resolve(); });
  expect(JSON.parse(localStorage.getItem("rbl.master-level.v1")!)).toBe(level);
  act(() => root.unmount());
  engineLevel = 1;
  setMasterLevel.mockClear();
  root = createRoot(host);
  await act(async () => { root.render(<Probe />); await Promise.resolve(); });
  expect(setMasterLevel).toHaveBeenCalledWith(level);
  expect(engineLevel).toBe(level);
  expect(master.level).toBe(level);
});
