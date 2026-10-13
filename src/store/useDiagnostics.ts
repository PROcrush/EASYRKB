/**
 * What the app is costing, for the title bar's readout.
 *
 * Readings are chained with `setTimeout` rather than run on an interval: an
 * interval that outlives the page keeps firing, and a reading is only worth
 * taking once the last one has come back.
 *
 * The readout must cost less than the budget it reports. At one reading a
 * second with ten frames timed per reading, it was the idle load: ten forced
 * composites, a process walk in the backend and a re-render every second
 * came to 0.8 % of a core in the app and 2.6 % in the webview against a
 * budget of 0.5 % (measured on a compiled 0.5.1 with the library loaded).
 * So: a reading every four seconds, and the frame rate from two frames
 * rather than ten — the interval between two consecutive frames is the
 * refresh period, which is all ten frames were averaging.
 */
import { useSyncExternalStore } from "react";

import { getBackend } from "@/ipc/client";
import type { Diagnostics } from "@/ipc/types";

/** Between readings. Five seconds is a readout that still moves, at a
 * fortieth of the idle cost a reading a second had. */
const PERIOD_MS = 5000;

/**
 * Frames timed per reading: two, the least that gives an interval. Every
 * frame asked for here is a frame the compositor has to draw, so this is the
 * whole cost of the FPS figure.
 */
const FRAMES = 2;

export interface AppCost extends Diagnostics {
  /** Frames a second, measured in the webview, or `null` before the first. */
  fps: number | null;
}

const NOTHING: AppCost = {
  audioLoad: 0,
  audioXruns: 0,
  cpu: 0,
  memoryMb: 0,
  threads: null,
  openFiles: null,
  gpu: null,
  fps: null,
};

/**
 * Times `FRAMES` frames and returns the rate they arrived at.
 *
 * The first frame's timestamp is the start, so the rate is over the gaps
 * between frames rather than from the moment of asking — which included the
 * wait for the first frame and read low.
 */
function measureFps(): Promise<number> {
  return new Promise((resolve) => {
    let start = 0;
    let seen = 0;
    const step = (now: number) => {
      seen += 1;
      if (seen === 1) {
        start = now;
        requestAnimationFrame(step);
        return;
      }
      if (seen < FRAMES) {
        requestAnimationFrame(step);
        return;
      }
      const elapsed = now - start;
      resolve(elapsed > 0 ? ((seen - 1) / elapsed) * 1000 : 0);
    };
    requestAnimationFrame(step);
  });
}

/**
 * One poller for the whole window, shared by whoever shows a figure.
 *
 * A store rather than a hook holding state in the shell: with `cost` a
 * value of `App`, every reading re-rendered the entire shell — the browser,
 * the deck, the tree — to change four short strings, and that render was
 * most of what the webview did all day. Now a reading changes this store,
 * and only the readout and the processor meter, which subscribe to it,
 * render again. The poller runs while anything subscribes and stops when
 * nothing does.
 */
let current: AppCost = NOTHING;
const listeners = new Set<() => void>();
let live = false;
let timer: ReturnType<typeof setTimeout> | null = null;

function publish(next: AppCost) {
  current = next;
  for (const listener of listeners) listener();
}

async function read() {
  try {
    const backend = await getBackend();
    const [sample, fps] = await Promise.all([backend.appDiagnostics(), measureFps()]);
    if (live) publish({ ...sample, fps });
  } catch {
    // A build with no backend behind it simply shows nothing.
    if (live) publish(NOTHING);
  }
  if (live) timer = setTimeout(() => void read(), PERIOD_MS);
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  if (!live) {
    live = true;
    void read();
  }
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) {
      live = false;
      if (timer !== null) clearTimeout(timer);
      timer = null;
    }
  };
}

function snapshot(): AppCost {
  return current;
}

/** What the app is costing now, updated every few seconds while mounted. */
export function useAppCost(): AppCost {
  return useSyncExternalStore(subscribe, snapshot, snapshot);
}

/** `1.2 GB`, `312 MB` — the readout has no room for six digits of bytes. */
export function formatMemory(mb: number): string {
  if (!Number.isFinite(mb) || mb <= 0) return "—";
  return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${Math.round(mb)} MB`;
}

/** A whole number, or a dash where the platform will not say. */
export function formatCount(value: number | null): string {
  return value === null || !Number.isFinite(value) ? "—" : String(Math.round(value));
}

/**
 * `12%`, or a dash.
 *
 * A tenth is kept below 10%, where rounding to a whole number reports a quiet
 * app as `0%` — a reading indistinguishable from a broken one, which is how
 * an idle 0.15% used to read.
 */
export function formatPercent(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return "—";
  if (value > 0 && value < 10) return `${value.toFixed(1)}%`;
  return `${Math.round(value)}%`;
}
