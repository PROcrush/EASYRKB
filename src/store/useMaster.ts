/**
 * The master level and its meters, for the top bar.
 *
 * Read from the same tick the decks are: the engine emits one, ten times a
 * second, and this picks the three fields out of it rather than asking for
 * them separately. The level is written back through a command, and comes
 * home on the next tick — what the callback actually applied, not what it was
 * asked for.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { getBackend } from "@/ipc/client";
import type { VuMeterMode } from "@/lib/preferences";
import { emptyVu, VuMeter, type VuDisplay } from "@/lib/vuMeter";
import { FULL_GAIN } from "@/lib/volume";

/**
 * How fast a meter falls, in decibels a second.
 *
 * A peak programme meter's own figure: 20 dB in a second reads as a needle
 * settling. Per frame it was too fast to see — 20 % of what was left every
 * thirtieth of a second is a bar that has gone before the eye arrives.
 */
const FALL_DB_PER_SECOND = 20;

/** The longest step the fall is applied over, so a stall is not a jump to nil. */
const MAX_STEP_SECONDS = 0.25;

/**
 * Where the fall gives up and calls it silence.
 *
 * An exponential decay approaches nothing without ever arriving, so a meter
 * left to it keeps a sliver of bar lit for good. -60 dB is under a pixel of a
 * 34-point meter and well below anything audible, so it reads as off — and it
 * gives the fall an end to reach, which is what lets the animation stop.
 */
const SILENCE = 0.001;

/**
 * The value a meter shows next: the reading, or the last one fallen.
 *
 * Fast attack, slow release. A meter that simply took each reading flickers,
 * and the loud moment — the one worth seeing — is gone before the eye is.
 *
 * The fall is measured in time rather than in frames, so it looks the same
 * whatever rate the meters arrive at and does not race if a frame is dropped.
 */
export function nextPeak(shown: number, reading: number, seconds: number): number {
  const safe = Number.isFinite(reading) ? Math.max(reading, 0) : 0;
  const previous = Number.isFinite(shown) ? Math.max(shown, 0) : 0;
  const step = Number.isFinite(seconds) ? Math.min(Math.max(seconds, 0), MAX_STEP_SECONDS) : 0;
  const held = previous * 10 ** ((-FALL_DB_PER_SECOND * step) / 20);
  const next = Math.min(Math.max(safe, held), 1);
  return next < SILENCE ? 0 : next;
}

/**
 * The reduction readout's next value: the reading, or the last one fallen.
 *
 * In decibels rather than a fraction, so the fall is a straight line at the
 * meters' own rate and a reading of nothing is reached rather than approached.
 */
export function nextReduction(shown: number, reading: number, seconds: number): number {
  const safe = Number.isFinite(reading) ? Math.max(reading, 0) : 0;
  const previous = Number.isFinite(shown) ? Math.max(shown, 0) : 0;
  const step = Number.isFinite(seconds) ? Math.min(Math.max(seconds, 0), MAX_STEP_SECONDS) : 0;
  const held = previous - FALL_DB_PER_SECOND * step;
  return Math.max(safe, held, 0);
}

/**
 * How long a meter waits for the next reading before it falls on its own.
 *
 * The engine's ticker stops after publishing the RMS tail. Display holds can
 * outlast those readings, so the meter finishes its own fall once IPC stops.
 *
 * Three tick periods at 30 Hz: long enough that a reading arriving late is not
 * mistaken for the end of the music.
 */
const SILENT_AFTER_MS = 100;

export interface Master {
  vu: VuDisplay;
  /** 0 to +2 dB. */
  level: number;
  peakLeft: number;
  peakRight: number;
  /** How far the limiter turned the sum down over the last tick, in dB. */
  reduction: number;
  setLevel: (level: number) => void;
}

const LEVEL_STORAGE_KEY = "rbl.master-level.v1";

/** `null` when nothing was ever saved, so the engine's own default is left alone. */
function loadLevel(): number | null {
  try {
    const raw = localStorage.getItem(LEVEL_STORAGE_KEY);
    if (raw === null) return null;
    const value: unknown = JSON.parse(raw);
    return typeof value === "number" && Number.isFinite(value) ? Math.min(FULL_GAIN, Math.max(0, value)) : null;
  } catch {
    return null;
  }
}

export function useMaster(mode: VuMeterMode = "normal"): Master {
  // The knob's placeholder until the backend answers, and what a first run
  // (nothing remembered yet) restores to: the engine's own idle default, not
  // full, which `show(now.master)` below overwrites either way.
  const remembered = useRef(loadLevel());
  const [state, setState] = useState(() => ({ level: remembered.current ?? 1, peakLeft: 0, peakRight: 0, reduction: 0, vu: emptyVu(mode) }));
  const wantedLevel = useRef(state.level);

  useEffect(() => {
    let live = true;
    const meterLeft = new VuMeter(mode);
    const meterRight = new VuMeter(mode);
    let vu = emptyVu(mode);
    const advance = (peakLeft: number, peakRight: number, rmsLeft: number, rmsRight: number, ms: number) => {
      const left = meterLeft.step(peakLeft, rmsLeft, ms);
      const right = meterRight.step(peakRight, rmsRight, ms);
      if (left !== vu.left || right !== vu.right) vu = { mode, left, right };
    };
    let stop: (() => void) | undefined;
    // The shown peaks, which are what the fall works on. They are held here
    // rather than read back out of the state so that the fall can ask whether
    // it has finished without reaching into a render.
    let left = 0;
    let right = 0;
    // The limiter's reduction falls the same way, so a single kick's dip is
    // seen rather than gone before the next reading.
    let reduction = 0;
    let last = performance.now();
    let falling: number | undefined;
    let quiet: ReturnType<typeof setTimeout> | undefined;

    const show = (level?: number) => {
      setState((current) => {
        const next = level ?? current.level;
        return current.level === next &&
          current.peakLeft === left &&
          current.peakRight === right &&
          current.reduction === reduction && current.vu === vu
          ? current
          : { level: next, peakLeft: left, peakRight: right, reduction, vu };
      });
    };

    /** The fall, once the readings have stopped coming; ends at silence. */
    const fall = (now: number) => {
      falling = undefined;
      if (!live) return;
      const elapsed = (now - last) / 1000;
      last = now;
      advance(0, 0, 0, 0, elapsed * 1000);
      left = nextPeak(left, 0, elapsed);
      right = nextPeak(right, 0, elapsed);
      reduction = nextReduction(reduction, 0, elapsed);
      show();
      // Nothing left to fall, so nothing left to draw: the frames stop here
      // rather than running on against an idle window.
      if (left > 0 || right > 0 || reduction > 0 || meterLeft.active || meterRight.active) falling = requestAnimationFrame(fall);
    };

    /** Hands the meters over to the fall if the next reading does not come. */
    const watch = () => {
      if (quiet !== undefined) clearTimeout(quiet);
      quiet = setTimeout(() => {
        quiet = undefined;
        if (!live || falling !== undefined || (left === 0 && right === 0 && reduction === 0 && !meterLeft.active && !meterRight.active)) {
          return;
        }
        last = performance.now();
        falling = requestAnimationFrame(fall);
      }, SILENT_AFTER_MS);
    };

    void (async () => {
      const backend = await getBackend();
      if (!live) return;
      // Nothing to restore: leave the engine's own default alone rather than
      // pushing the knob's full-scale placeholder onto it.
      if (remembered.current !== null) {
        try {
          await backend.setMasterLevel(remembered.current);
        } catch {
          // Keep the remembered knob position if the audio device is unavailable.
        }
      }
      if (!live) return;
      // The meters come on their own beat, three times as often as the decks.
      const unlisten = backend.onMeters((meters) => {
        if (!live) return;
        // A reading is the music still playing, so the fall stands down and
        // the readings drive the meter again.
        if (falling !== undefined) {
          cancelAnimationFrame(falling);
          falling = undefined;
        }
        const now = performance.now();
        const elapsed = (now - last) / 1000;
        last = now;
        // A peak falls back rather than dropping: a meter that snaps to the
        // next reading flickers, and the loud moment is the one to see.
        advance(meters.peakLeft, meters.peakRight, meters.rmsLeft ?? 0, meters.rmsRight ?? 0, elapsed * 1000);
        left = nextPeak(left, meters.peakLeft, elapsed);
        right = nextPeak(right, meters.peakRight, elapsed);
        reduction = nextReduction(reduction, meters.reduction, elapsed);
        show(meters.master);
        watch();
      });
      if (!live) {
        unlisten();
        return;
      }
      stop = unlisten;
      // What it holds now, so a reload does not show the knob at the top.
      const now = await backend.deckState();
      if (!live) return;
      left = Math.min(Math.max(now.peakLeft, 0), 1);
      right = Math.min(Math.max(now.peakRight, 0), 1);
      reduction = nextReduction(0, now.reduction, 0);
      advance(left, right, 0, 0, 0);
      last = performance.now();
      show(now.master);
      // Opening onto a stopped engine is the same case as the music ending:
      // no reading will come, so the bars have to bring themselves down.
      watch();
    })();

    return () => {
      live = false;
      stop?.();
      if (quiet !== undefined) clearTimeout(quiet);
      if (falling !== undefined) cancelAnimationFrame(falling);
    };
  }, [mode]);

  const setLevel = useCallback((value: number) => {
    const level = Number.isFinite(value) ? Math.min(FULL_GAIN, Math.max(0, value)) : 1;
    wantedLevel.current = level;
    try {
      localStorage.setItem(LEVEL_STORAGE_KEY, JSON.stringify(level));
    } catch {
      // Storage failures must not prevent adjusting the volume.
    }
    // Locally first: a knob that waits a tenth of a second to move is a knob
    // that feels broken.
    setState((current) => ({ ...current, level }));
    void (async () => {
      try {
        const backend = await getBackend();
        await backend.setMasterLevel(level);
      } catch {
        // No engine behind this build; the knob still turns.
      }
    })();
  }, []);

  return useMemo(() => ({ ...state, setLevel }), [state, setLevel]);
}
