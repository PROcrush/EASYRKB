import { describe, expect, it } from "vitest";

import { EMPTY_FILTER, isNarrowing, pick, toSpecFilter, wholeBpm, type FilterState } from "./trackFilter";

function withBpm(values: number[], tolerancePct = 0, enabled = true): FilterState {
  return { ...EMPTY_FILTER, bpm: { enabled, values, tolerancePct } };
}

describe("pick", () => {
  it("a plain click picks one value", () => {
    expect(pick([128, 130], 126, false)).toEqual([126]);
  });

  it("the toggle modifier adds and removes", () => {
    expect(pick([128], 130, true)).toEqual([128, 130]);
    expect(pick([128, 130], 128, true)).toEqual([130]);
  });

  it("All clears the picks either way", () => {
    expect(pick([128, 130], null, false)).toEqual([]);
    expect(pick([128, 130], null, true)).toEqual([]);
  });
});

describe("toSpecFilter", () => {
  it("sends nothing while no column is ticked", () => {
    const picked: FilterState = { ...withBpm([128], 2, false), key: { enabled: false, values: ["Fm"] } };
    expect(isNarrowing(picked)).toBe(false);
    expect(toSpecFilter(picked, 12800)).toBeUndefined();
  });

  it("carries a ticked BPM column with the master player's BPM", () => {
    expect(toSpecFilter(withBpm([128], 2), 12600)).toEqual({
      bpm: { values: [128], tolerancePct: 2, masterBpmX100: 12600 },
    });
    // No deck loaded: the ± list has nothing to centre on.
    expect(toSpecFilter(withBpm([], 2), null)?.bpm?.masterBpmX100).toBeNull();
    expect(toSpecFilter(withBpm([], 2), 0)?.bpm?.masterBpmX100).toBeNull();
  });

  it("leaves a ticked set column at All off the wire", () => {
    const state: FilterState = {
      ...EMPTY_FILTER,
      key: { enabled: true, values: [] },
      rating: { enabled: true, values: [5] },
      color: { enabled: true, values: ["Red", "Aqua"] },
    };
    expect(toSpecFilter(state, null)).toEqual({ ratings: [5], colors: ["Red", "Aqua"] });
    // Every ticked column at All is no filter at all.
    expect(toSpecFilter({ ...EMPTY_FILTER, key: { enabled: true, values: [] } }, null)).toBeUndefined();
  });
});

describe("wholeBpm", () => {
  it("rounds to the nearest whole number", () => {
    expect(wholeBpm(12798)).toBe(128);
    expect(wholeBpm(12850)).toBe(129);
    expect(wholeBpm(0)).toBe(0);
  });
});
