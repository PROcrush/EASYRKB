/**
 * Where the playhead is between ticks.
 *
 * The engine sends one event ten times a second carrying both decks' frame
 * counters. Sixty a second would be pure IPC churn and the interface would
 * still have to interpolate to draw a smooth playhead, so it interpolates from
 * ten instead: each tick is an anchor, and every frame in between is that
 * anchor plus the time since it arrived.
 */

/** One deck as the engine last reported it. */
export interface Anchor {
  /** The frame counter at the moment the tick was sent. */
  frames: number;
  /** When the tick arrived here, from `performance.now()`. */
  at: number;
  /** The device rate, which is what `frames` counts in. */
  sampleRate: number;
  playing: boolean;
  /**
   * Bumped by the engine on every load and seek. A tick with a new generation
   * is a deliberate move, so the playhead snaps to it rather than sliding.
   */
  generation: number;
  /**
   * How fast the deck is running against real time. One until the tempo
   * control ships; the arithmetic is here now so the playhead does not have to
   * change when it does.
   */
  rate: number;
  /** A scheduled start, distinct from the arrival time of an ordinary tick. */
  startsAt?: number | undefined;
}

/** A stopped deck at the start of a track. */
export const NO_ANCHOR: Anchor = {
  frames: 0,
  at: 0,
  sampleRate: 0,
  playing: false,
  generation: 0,
  rate: 1,
};

/** Seconds into the track, `now` milliseconds into the tick. */
export function extrapolate(anchor: Anchor, now: number): number {
  if (anchor.sampleRate <= 0) return 0;
  const base = anchor.frames / anchor.sampleRate;
  if (!anchor.playing) return base;
  // Never backwards: a tick that arrives late must not rewind the head.
  const elapsed = Math.max(0, now - anchor.at) / 1000;
  return base + elapsed * anchor.rate;
}

/**
 * A correction larger than this is a seek, a load or a sync nudge rather than
 * drift, and is taken at once.
 */
export const SNAP_SECONDS = 0.25;

/** How quickly a small correction is absorbed; about 150 ms to close a gap. */
const EASE_MS = 50;

/**
 * The position to draw, easing towards `target` from where it was.
 *
 * Ticks arrive a few milliseconds early or late, so the extrapolated position
 * steps by a millisecond or two at each one. Drawn raw that is a visible
 * stutter in the playhead; eased, it is invisible.
 */
export function follow(shown: number, target: number, sinceMs: number, advanceSeconds = 0): number {
  // Advance with playback first; ease only the remaining clock correction.
  // Easing the moving position itself adds a permanent ~42 ms lag at 60 Hz.
  const predicted = shown + advanceSeconds;
  const gap = target - predicted;
  if (Math.abs(gap) > SNAP_SECONDS || sinceMs <= 0) return target;
  return predicted + gap * (1 - Math.exp(-sinceMs / EASE_MS));
}

/**
 * An anchor that stands still at `seconds`.
 *
 * A drag owns the playhead, and neither the deck's ticks nor the frame loop
 * may move it. The deck's own read head is not where the pointer is: it is
 * rate-limited so a drag stays audible, so it trails a fast hand and comes to
 * rest about a block past a still one. Anchoring on it and then extrapolating
 * at playback speed drew the head creeping forward between ticks and snapping
 * back on each one — a waveform jittering under a hand that was holding it
 * still.
 */
export function pinned(anchor: Anchor, seconds: number, now: number): Anchor {
  return {
    ...anchor,
    frames: anchor.sampleRate > 0 ? seconds * anchor.sampleRate : anchor.frames,
    at: now,
    playing: false,
  };
}
