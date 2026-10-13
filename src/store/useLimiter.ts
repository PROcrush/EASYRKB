/**
 * The master limiter's settings, remembered between sessions.
 *
 * The engine holds the live values but is built late and rebuilt on a device
 * change, so the setting of record lives here: read from `localStorage` when
 * the app starts, pushed to the backend at once so the first thing played has
 * it, and pushed again on every change. What the backend says it set is what
 * is shown — it clamps the numbers to what the limiter can do.
 */
import { useCallback, useEffect, useRef, useState } from "react";

import { getBackend } from "@/ipc/client";
import type { Limiter } from "@/ipc/types";

const STORAGE_KEY = "rbl.limiter.v1";

/** The engine's own defaults, restated so a first run shows them at once. */
export const DEFAULT_LIMITER: Limiter = { enabled: false, inputGainDb: -4, ceilingDb: 0, releaseMs: 250 };

/** The ranges the engine accepts; anything outside is clamped by it too. */
export const INPUT_GAIN_DB = { min: -24, max: 24, step: 0.1 } as const;
export const CEILING_DB = { min: -12, max: 0, step: 0.1 } as const;
export const RELEASE_MS = { min: 10, max: 1000, step: 10 } as const;

function clampTo(value: unknown, min: number, max: number, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.min(Math.max(value, min), max)
    : fallback;
}

/** A stored value made safe: a half-written or hand-edited one falls back. */
export function sanitise(raw: unknown): Limiter {
  const candidate = (raw ?? {}) as Partial<Record<keyof Limiter, unknown>>;
  return {
    enabled: typeof candidate.enabled === "boolean" ? candidate.enabled : DEFAULT_LIMITER.enabled,
    inputGainDb: clampTo(candidate.inputGainDb, INPUT_GAIN_DB.min, INPUT_GAIN_DB.max, DEFAULT_LIMITER.inputGainDb),
    ceilingDb: clampTo(candidate.ceilingDb, CEILING_DB.min, CEILING_DB.max, DEFAULT_LIMITER.ceilingDb),
    releaseMs: clampTo(candidate.releaseMs, RELEASE_MS.min, RELEASE_MS.max, DEFAULT_LIMITER.releaseMs),
  };
}

function load(): Limiter {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return sanitise(raw === null ? null : JSON.parse(raw));
  } catch {
    // Private browsing, a disabled store, or a half-written value: the
    // defaults are what the engine would use anyway.
    return DEFAULT_LIMITER;
  }
}

export interface LimiterControl {
  limiter: Limiter;
  /** Any subset; the rest is kept. */
  set: (change: Partial<Limiter>) => void;
}

export function useLimiter(): LimiterControl {
  const [limiter, setLimiter] = useState<Limiter>(load);
  // The latest value outside a render, so a change can be built on it without
  // a side effect inside a state updater.
  const latest = useRef(limiter);
  latest.current = limiter;
  // Which push is the latest, so a slow reply does not overwrite a newer one.
  const sequence = useRef(0);

  const push = useCallback(async (wanted: Limiter) => {
    const mine = ++sequence.current;
    try {
      const backend = await getBackend();
      const set = await backend.setMasterLimiter(wanted);
      if (mine !== sequence.current) return;
      // What the engine could do, which is what to show and to remember.
      setLimiter((current) =>
        current.enabled === set.enabled &&
        current.inputGainDb === set.inputGainDb &&
        current.ceilingDb === set.ceilingDb &&
        current.releaseMs === set.releaseMs
          ? current
          : set,
      );
    } catch {
      // No engine behind this build; the controls still move and are still
      // remembered for when there is.
    }
  }, []);

  // Once, at start: the engine must have the remembered setting before the
  // first thing plays, not when Settings is next opened.
  useEffect(() => {
    void push(load());
  }, [push]);

  useEffect(() => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(limiter));
    } catch {
      // Not being able to remember it is not a reason to break the session.
    }
  }, [limiter]);

  const set = useCallback(
    (change: Partial<Limiter>) => {
      const next = sanitise({ ...latest.current, ...change });
      latest.current = next;
      // Locally first: a control that waits for the engine to answer is a
      // control that feels broken.
      setLimiter(next);
      void push(next);
    },
    [push],
  );

  return { limiter, set };
}
