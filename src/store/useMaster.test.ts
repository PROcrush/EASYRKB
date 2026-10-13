import { describe, expect, it } from "vitest";

import { nextPeak } from "./useMaster";

/** One meter event at thirty a second. */
const FRAME = 1 / 30;

describe("nextPeak", () => {
  it("takes a louder reading at once", () => {
    // Fast attack: the transient is the thing worth seeing.
    expect(nextPeak(0.2, 0.9, FRAME)).toBe(0.9);
  });

  it("falls at twenty decibels a second when the reading is quieter", () => {
    // A peak meter's own figure, and measured in time rather than in frames:
    // a tenth of the amplitude after a second is 20 dB down.
    let shown = 1;
    for (let i = 0; i < 30; i++) shown = nextPeak(shown, 0, FRAME);
    expect(shown).toBeCloseTo(0.1, 2);
  });

  it("falls the same however often it is asked", () => {
    // A fifth of a second in one step and in six must land in the same place,
    // or a dropped frame makes the needle drop with it. Only up to the stall
    // clamp, which is the next test.
    let stepped = 1;
    for (let i = 0; i < 6; i++) stepped = nextPeak(stepped, 0, FRAME);
    expect(stepped).toBeCloseTo(nextPeak(1, 0, 0.2), 4);
  });

  it("does not fall off a cliff after a stall", () => {
    // A window left in the background for a minute should come back to a
    // meter, not to a bar that jumped to nothing between two readings.
    expect(nextPeak(1, 0, 60)).toBeCloseTo(nextPeak(1, 0, 0.25), 6);
  });

  it("reaches nothing rather than a sliver that never goes", () => {
    // An exponential fall approaches nothing without arriving, so below -60 dB
    // it is called silence. Otherwise the bar keeps a lit edge for good and
    // the fall has no end to stop at.
    let shown = 1;
    for (let i = 0; i < 300; i++) shown = nextPeak(shown, 0, FRAME);
    expect(shown).toBe(0);
  });

  it("never leaves the bar, whatever it is handed", () => {
    for (const [shown, reading, seconds] of [
      [Number.NaN, 0.5, FRAME],
      [0.5, Number.NaN, FRAME],
      [0.5, 0.5, Number.NaN],
      [-1, -1, FRAME],
      [2, 3, FRAME],
    ] as const) {
      const next = nextPeak(shown, reading, seconds);
      expect(next).toBeGreaterThanOrEqual(0);
      expect(next).toBeLessThanOrEqual(1);
    }
  });
});
