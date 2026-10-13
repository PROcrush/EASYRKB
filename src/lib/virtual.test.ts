import { describe, expect, it } from "vitest";
import { planFetches, visibleWindow } from "./virtual";

describe("visibleWindow", () => {
  it("covers the viewport plus overscan", () => {
    expect(visibleWindow(0, 250, 25, 1000, 8)).toEqual({ start: 0, end: 18 });
    expect(visibleWindow(500, 250, 25, 1000, 8)).toEqual({ start: 12, end: 38 });
  });
  it("clamps at both ends rather than requesting past the data", () => {
    expect(visibleWindow(0, 250, 25, 5, 8)).toEqual({ start: 0, end: 5 });
    const w = visibleWindow(24_800, 250, 25, 1000, 8);
    expect(w.end).toBe(1000);
    expect(w.start).toBeLessThanOrEqual(w.end);
  });
  it("is empty for an empty view", () => {
    expect(visibleWindow(0, 250, 25, 0)).toEqual({ start: 0, end: 0 });
  });
});

describe("planFetches", () => {
  it("skips pages already in flight", () => {
    expect(planFetches([1, 2, 3], new Set([2]))).toEqual([1, 3]);
  });
  it("caps requests per tick so a flick cannot queue hundreds", () => {
    expect(planFetches([1, 2, 3, 4, 5, 6], new Set(), 4)).toHaveLength(4);
  });
});
