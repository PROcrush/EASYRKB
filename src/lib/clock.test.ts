import { describe, expect, it } from "vitest";

import { extrapolate, follow, NO_ANCHOR, pinned, SNAP_SECONDS, type Anchor } from "./clock";

const anchor = (over: Partial<Anchor> = {}): Anchor => ({
  ...NO_ANCHOR,
  frames: 44_100,
  at: 1_000,
  sampleRate: 44_100,
  playing: true,
  ...over,
});

describe("extrapolate", () => {
  it("reads the frame counter as seconds", () => {
    expect(extrapolate(anchor({ playing: false }), 1_000)).toBeCloseTo(1);
  });

  it("runs on with real time while the deck plays", () => {
    // Half a second after a tick that said one second in.
    expect(extrapolate(anchor(), 1_500)).toBeCloseTo(1.5);
  });

  it("stands still while the deck is paused", () => {
    expect(extrapolate(anchor({ playing: false }), 5_000)).toBeCloseTo(1);
  });

  it("never runs backwards when a tick arrives late", () => {
    expect(extrapolate(anchor(), 900)).toBeCloseTo(1);
  });

  it("is zero before a device has said what rate it runs at", () => {
    expect(extrapolate(NO_ANCHOR, 10_000)).toBe(0);
  });

  it("follows the deck's own rate, for when tempo ships", () => {
    expect(extrapolate(anchor({ rate: 1.08 }), 2_000)).toBeCloseTo(1 + 1.08);
  });
});

describe("follow", () => {
  it("adds no steady playback lag across frame rates and tempos", () => {
    for (const fps of [30, 60, 120]) {
      for (const rate of [0.5, 1, 1.08, 2]) {
        let shown = 0;
        for (let frame = 1; frame <= fps * 3; frame++) {
          const target = frame * rate / fps;
          shown = follow(shown, target, 1000 / fps, rate / fps);
          expect(shown).toBeCloseTo(target, 9);
        }
      }
    }
  });
  it("closes a small gap gradually rather than jumping", () => {
    const next = follow(1.0, 1.01, 16);
    expect(next).toBeGreaterThan(1.0);
    expect(next).toBeLessThan(1.01);
  });

  it("gets there: a gap left alone is gone within a third of a second", () => {
    let shown = 1.0;
    // Twenty frames at 60 Hz is 320 ms, or six time constants.
    for (let frame = 0; frame < 20; frame++) shown = follow(shown, 1.01, 16);
    expect(shown).toBeCloseTo(1.01, 4);
  });

  it("snaps rather than sliding across a seek", () => {
    // Sliding a whole second would draw the playhead crossing the track.
    expect(follow(1.0, 60.0, 16)).toBe(60.0);
    expect(follow(60.0, 1.0, 16)).toBe(1.0);
  });

  it("takes the target when no time has passed", () => {
    expect(follow(1.0, 1.05, 0)).toBe(1.05);
  });

  it("treats exactly the threshold as drift, not a seek", () => {
    expect(follow(1.0, 1.0 + SNAP_SECONDS, 16)).toBeLessThan(1.0 + SNAP_SECONDS);
  });
});

describe("pinned", () => {
  it("stands still, however much time passes", () => {
    // The one that bit: a drag on a playing deck kept extrapolating at
    // playback speed, so the waveform crept forward under a hand holding it
    // still and snapped back on the next tick.
    const held = pinned(anchor({ playing: true, sampleRate: 44_100 }), 2, 5_000);
    expect(extrapolate(held, 5_000)).toBeCloseTo(2, 6);
    expect(extrapolate(held, 5_500)).toBeCloseTo(2, 6);
  });

  it("takes the seconds it is given, not the frames it had", () => {
    const held = pinned(anchor({ sampleRate: 44_100 }), 30, 0);
    expect(held.frames).toBe(30 * 44_100);
  });

  it("keeps what it had when the rate is not known yet", () => {
    // Before the first tick there is no sample rate, and converting against
    // zero would pin the head to the start of the track.
    const before = anchor({ sampleRate: 0, frames: 44_100 });
    expect(pinned(before, 30, 0).frames).toBe(44_100);
  });
});
