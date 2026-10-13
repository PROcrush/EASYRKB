import { describe, expect, it } from "vitest";

import { BEATS_PER_BAR, NO_BEATS } from "./player";
import {
  barAt, beatNudgeFor, beatWait, inPhaseAt, loopPeriodBeats, MAX_TEMPO, MIN_TEMPO, nudgeFor, syncTo, tempoFor, type Deck,
} from "./sync";

/** A grid at a steady `bpm`, starting `offsetMs` in. */
function grid(bpm: number, beats: number, offsetMs = 0) {
  const times = new Uint32Array(beats);
  const numbers = new Uint8Array(beats);
  const tempos = new Uint16Array(beats).fill(Math.round(bpm * 100));
  const beatMs = 60_000 / bpm;
  for (let i = 0; i < beats; i++) {
    times[i] = Math.round(offsetMs + i * beatMs);
    numbers[i] = (i % BEATS_PER_BAR) + 1;
  }
  return { times, numbers, tempos };
}

function deck(bpm: number, position: number, offsetMs = 0): Deck {
  return { bpmX100: Math.round(bpm * 100), position, grid: grid(bpm, 256, offsetMs) };
}

describe("sync follows timestamped beats", () => {
  const changing: Deck = {
    bpmX100: 12000,
    position: 1.4,
    grid: {
      times: Uint32Array.from([0, 500, 1100, 1800, 2600, 3400]),
      numbers: Uint8Array.from([1, 2, 3, 4, 1, 2]),
      tempos: Uint16Array.from([12000, 10000, 8571, 7500, 7500, 7500]),
    },
  };

  it("uses the current beat interval to align a changing-tempo master", () => {
    expect(beatNudgeFor(changing, deck(120, 4))).toBeCloseTo((0.3 / 0.7) * 0.5, 6);
    expect(beatWait(changing)).toBeCloseTo(0.4, 6);
  });

  it("matches the current grid tempo instead of the track's average BPM", () => {
    // Grid tempos are stored in hundredths of a BPM.
    expect(tempoFor(changing, deck(120, 4), { doubleHalf: false })).toBeCloseTo(8571 / 12000, 6);
  });

  it("waits for the first timestamped beat during a leading intro", () => {
    expect(beatWait(deck(120, 0, 300))).toBeCloseTo(0.3, 6);
  });
  it("aligns the bar using individual beat timestamps through a tempo change", () => {
    const follower = deck(120, 4);
    // Beat 3 + 3/7 of a beat; the nearest matching bar phase is back 11/7 beats.
    expect(nudgeFor(changing, follower)).toBeCloseTo((-11 / 7) * 0.5, 6);
    const move = nudgeFor(follower, changing);
    expect(changing.position + move).toBeCloseTo(2.6, 6);
  });
  it("keeps half/double BPM beats aligned without repeated corrective jumps", () => {
    // Beat timestamps have millisecond precision.
    expect(beatNudgeFor(deck(70, 4.2), deck(140, 4.2))).toBeCloseTo(0, 3);
    expect(beatNudgeFor(deck(140, 4.2), deck(70, 4.2))).toBeCloseTo(0, 3);
  });
  it("uses the measured beat interval to recognize a short loop on a changing grid", () => {
    const at = inPhaseAt(deck(120, 4.1), changing, 1.2, { inSeconds: 1.1, outSeconds: 1.45 });
    expect(at).not.toBeCloseTo(1.2, 6);
    expect(at).toBeGreaterThanOrEqual(1.1);
    expect(at).toBeLessThan(1.45);
  });
  it("matches the tempo at the landing point when bar alignment crosses a tempo boundary", () => {
    const result = syncTo(deck(120, 4), changing, { doubleHalf: false });
    expect(changing.position + result.nudge).toBeCloseTo(2.6, 6);
    expect(result.tempo).toBeCloseTo(120 / 75, 6);
  });
  it("continues the last local tempo beyond a partial grid", () => {
    expect(beatWait({ ...changing, position: 4.3 })).toBeCloseTo(0.7, 6);
  });
});

describe("tempoFor", () => {
  it("is the ratio of the two BPMs", () => {
    // 124 played at 128 is 128/124.
    expect(tempoFor(deck(128, 0), deck(124, 0))).toBeCloseTo(128 / 124, 6);
    expect(tempoFor(deck(124, 0), deck(128, 0))).toBeCloseTo(124 / 128, 6);
  });

  it("leaves a deck alone when either side has no BPM", () => {
    // Syncing to a track nobody has analysed would be playing it at a ratio of
    // two numbers that mean nothing.
    const unknown: Deck = { bpmX100: 0, position: 0, grid: NO_BEATS };
    expect(tempoFor(deck(128, 0), unknown)).toBe(1);
    expect(tempoFor(unknown, deck(128, 0))).toBe(1);
  });

  it("will not pull a deck further than it can go", () => {
    // Half time and double time are the ends of the stretcher's range, so a
    // 70 to 174 match comes back clamped rather than as a ratio nothing can
    // play — once the double/half match is turned off in Preferences.
    const off = { doubleHalf: false };
    expect(tempoFor(deck(174, 0), deck(70, 0), off)).toBe(MAX_TEMPO);
    expect(tempoFor(deck(70, 0), deck(174, 0), off)).toBe(MIN_TEMPO);
  });

  it("follows the tempo the leader is playing at, not the one on its file", () => {
    // A 128 nudged up to 131.2 is followed at 131.2.
    expect(tempoFor({ ...deck(128, 0), tempo: 1.025 }, deck(128, 0))).toBeCloseTo(1.025, 6);
    expect(tempoFor(deck(128, 0), deck(128, 0))).toBe(1);
  });

  it("takes a double or half BPM as the same tempo, unless told not to", () => {
    // 140 beside 70 is a match at twice the leader's tempo — the follower
    // stays at its own speed rather than being slowed to half.
    expect(tempoFor(deck(70, 0), deck(140, 0))).toBe(1);
    expect(tempoFor(deck(140, 0), deck(70, 0))).toBe(1);
    // 174 beside 70: the half, 1.24, is nearer to its own speed than 2.49.
    expect(tempoFor(deck(174, 0), deck(70, 0))).toBeCloseTo(1.2428, 3);
    // Two tempos a few percent apart are pulled the short way as before.
    expect(tempoFor(deck(128, 0), deck(126, 0))).toBeCloseTo(128 / 126, 6);
    expect(tempoFor(deck(70, 0), deck(140, 0), { doubleHalf: false })).toBe(MIN_TEMPO);
  });
});

describe("syncTo", () => {
  it("BPM SYNC matches the tempo and leaves the playhead where it is", () => {
    const leadGrid = {
      times: Uint32Array.from([0, 500, 1000, 1500, 2000]),
      numbers: Uint8Array.from([1, 2, 3, 4, 1]),
      tempos: Uint16Array.from([12_000, 12_000, 12_000, 12_000, 12_000]),
    };
    const leader: Deck = { bpmX100: 12000, position: 0, grid: leadGrid };
    const follower = deck(128, 0.25);
    expect(syncTo(leader, follower, { type: "beat" }).nudge).not.toBe(0);
    expect(syncTo(leader, follower, { type: "bpm" })).toEqual({ tempo: 12000 / 12800, nudge: 0 });
  });
});

describe("barAt", () => {
  it("finds the downbeat of the bar the playhead is in", () => {
    // 120 BPM: a beat every half second, a bar every two.
    const at = barAt(deck(120, 5.2), 5.2);
    expect(at?.start).toBeCloseTo(4, 2);
    expect(at?.length).toBeCloseTo(2, 2);
  });

  it("reads the grid rather than the BPM, so a shifted track is still right", () => {
    // The same tempo, starting 300 ms in: every bar is 300 ms later than the
    // arithmetic would put it.
    const shifted = deck(120, 5.5, 300);
    const at = barAt(shifted, 5.5);
    expect(at?.start).toBeCloseTo(4.3, 2);
  });

  it("falls back to the BPM where the grid does not reach", () => {
    const ungridded: Deck = { bpmX100: 12_000, position: 9, grid: NO_BEATS };
    const at = barAt(ungridded, 9);
    expect(at?.length).toBeCloseTo(2, 6);
    expect(at?.start).toBeCloseTo(8, 6);
  });

  it("has nothing to say about a track with neither", () => {
    expect(barAt({ bpmX100: 0, position: 3, grid: NO_BEATS }, 3)).toBeNull();
  });
});

describe("nudgeFor", () => {
  it("is nothing when the two are already on the bar together", () => {
    expect(nudgeFor(deck(128, 4), deck(128, 4))).toBeCloseTo(0, 6);
  });

  it("moves the follower forward when it is behind", () => {
    // 120 BPM, a two-second bar. The leader is half a beat into its bar and
    // the follower has not reached its downbeat yet by a quarter second.
    const nudge = nudgeFor(deck(120, 4.25), deck(120, 4.0));
    expect(nudge).toBeCloseTo(0.25, 3);
  });

  it("takes the short way round rather than the long one", () => {
    // Three beats behind is one beat ahead: the ear hears the beat, not which
    // of the four it is.
    const nudge = nudgeFor(deck(120, 4.0), deck(120, 5.5));
    expect(nudge).toBeCloseTo(0.5, 3);
    expect(Math.abs(nudge)).toBeLessThanOrEqual(1.0001);
  });

  it("never asks for more than half a bar", () => {
    for (let at = 0; at < 2; at += 0.05) {
      const nudge = nudgeFor(deck(120, 4), deck(120, 4 + at));
      expect(Math.abs(nudge)).toBeLessThanOrEqual(1.0001);
    }
  });

  it("leaves a deck alone when there is no grid and no BPM to use", () => {
    const unknown: Deck = { bpmX100: 0, position: 1, grid: NO_BEATS };
    expect(nudgeFor(deck(128, 4), unknown)).toBe(0);
    expect(nudgeFor(unknown, deck(128, 4))).toBe(0);
  });
});

describe("syncTo", () => {
  it("brings a deck to the leader's tempo and onto its bar in one answer", () => {
    const leader = deck(128, 10);
    const follower = deck(124, 7.3);
    const { tempo, nudge } = syncTo(leader, follower);
    expect(tempo).toBeCloseTo(128 / 124, 6);

    // Applying the nudge puts the follower where the leader is in its own bar.
    const moved = { ...follower, position: follower.position + nudge };
    const into = (d: Deck) => {
      const bar = barAt(d, d.position);
      return bar ? (d.position - bar.start) / bar.length : 0;
    };
    expect(into(moved)).toBeCloseTo(into(leader), 3);
  });
});

describe("beatNudgeFor", () => {
  it("lines the follower's beat up with the leader's, whichever beat of the bar", () => {
    // 120 BPM: a half-second beat. The leader is a quarter beat past a beat
    // and the follower is on one, so the follower moves an eighth of a second
    // forward — not the two and a quarter beats a bar match would ask for.
    expect(beatNudgeFor(deck(120, 4.125), deck(120, 5.0))).toBeCloseTo(0.125, 3);
    // The short way round: three quarters past a beat is a quarter before the next.
    expect(beatNudgeFor(deck(120, 4.375), deck(120, 5.0))).toBeCloseTo(-0.125, 3);
    expect(beatNudgeFor(deck(120, 4.0), deck(120, 5.5))).toBeCloseTo(0, 3);
  });

  it("never asks for more than half a beat, and nothing without a grid or a BPM", () => {
    for (let at = 0; at < 1; at += 0.03) {
      expect(Math.abs(beatNudgeFor(deck(120, 4), deck(120, 4 + at)))).toBeLessThanOrEqual(0.2501);
    }
    const unknown: Deck = { bpmX100: 0, position: 1, grid: NO_BEATS };
    expect(beatNudgeFor(unknown, deck(128, 4))).toBe(0);
  });

  it("measures the phase in a part of a beat for a short loop", () => {
    // A half-beat loop at 120 BPM repeats every quarter second. The leader is
    // a quarter second past a beat, which is on the loop's period, so the
    // follower on a beat does not move.
    expect(beatNudgeFor(deck(120, 4.25), deck(120, 5.0), 0.5)).toBeCloseTo(0, 3);
    expect(beatNudgeFor(deck(120, 4.3), deck(120, 5.0), 0.5)).toBeCloseTo(0.05, 3);
  });
});

describe("inPhaseAt", () => {
  // 120 BPM throughout: a half-second beat, a two-second bar.
  const leader = deck(120, 4.1);

  it("puts a move on the leader's beat, half a beat away at most", () => {
    // A hot cue on a beat, with the leader a tenth of a second past one.
    expect(inPhaseAt(leader, deck(120, 0), 8, null)).toBeCloseTo(8.1, 3);
    // A FINE jump of a tenth of a beat goes back onto the beat.
    expect(inPhaseAt(leader, deck(120, 0), 8.15, null)).toBeCloseTo(8.1, 3);
  });

  it("keeps a landing near the out point inside the loop, by whole loops", () => {
    // A one-beat loop from 8 to 8.5. The leader asks for 0.1 s past a beat,
    // so a head at 8.49 goes forward past the out point and comes back in.
    const at = inPhaseAt(leader, deck(120, 0), 8.49, { inSeconds: 8, outSeconds: 8.5 });
    expect(at).toBeCloseTo(8.1, 3);
  });

  it("measures a half-beat loop in its own period", () => {
    // The leader at 0.1 s past a beat is 0.1 s into a quarter-second period.
    const at = inPhaseAt(leader, deck(120, 0), 8.2, { inSeconds: 8, outSeconds: 8.25 });
    expect(at).toBeCloseTo(8.1, 3);
  });

  it("leaves a loop that cannot stay in phase alone", () => {
    // One and a half beats: each repeat is half a beat off.
    expect(inPhaseAt(leader, deck(120, 0), 8.3, { inSeconds: 8, outSeconds: 8.75 })).toBe(8.3);
  });
});

describe("loopPeriodBeats", () => {
  it("is one beat for whole beats, the length for a part beat, and null for the rest", () => {
    expect(loopPeriodBeats(2, 0.5)).toBe(1);
    expect(loopPeriodBeats(0.5, 0.5)).toBe(1);
    expect(loopPeriodBeats(0.25, 0.5)).toBe(0.5);
    expect(loopPeriodBeats(0.125, 0.5)).toBe(0.25);
    expect(loopPeriodBeats(0.75, 0.5)).toBeNull();
    expect(loopPeriodBeats(0.3, 0.5)).toBeNull();
    expect(loopPeriodBeats(1, 0)).toBeNull();
  });
});

describe("beatWait", () => {
  it("is the time to the leader's next beat at the tempo it is playing", () => {
    // 120 BPM, a half-second beat, a quarter beat past one: a quarter second.
    expect(beatWait(deck(120, 4.125))).toBeCloseTo(0.375, 3);
    // Played at 1.25 times, that quarter of a beat passes in less time.
    expect(beatWait({ ...deck(120, 4.125), tempo: 1.25 })).toBeCloseTo(0.375 / 1.25, 3);
    // On the beat already: the whole of the next one.
    expect(beatWait(deck(120, 4.0))).toBeCloseTo(0.5, 3);
    expect(beatWait({ bpmX100: 0, position: 1, grid: NO_BEATS })).toBeNull();
  });
});

