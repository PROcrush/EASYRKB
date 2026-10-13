import { describe, expect, it } from "vitest";

import { backingSize, MAX_RATIO, MAX_WIDTH, STEP } from "./canvasSize";

describe("backingSize", () => {
  it("follows the element rather than a fixed number", () => {
    expect(backingSize(400, 14, 1).width).toBe(400);
    expect(backingSize(1200, 14, 1).width).toBe(1200);
  });

  it("doubles for a retina display", () => {
    expect(backingSize(400, 14, 2)).toEqual({ width: 800, height: 28 });
  });

  it("stops at 2x, because a 14-pixel strip cannot show more", () => {
    expect(backingSize(400, 14, 3)).toEqual(backingSize(400, 14, MAX_RATIO));
  });

  it("quantises the width so a drag does not reallocate on every pixel", () => {
    const a = backingSize(401, 14, 1).width;
    const b = backingSize(400 + STEP - 1, 14, 1).width;
    expect(a).toBe(b);
    expect(a % STEP).toBe(0);
  });

  it("never asks for a buffer a layout bug could make enormous", () => {
    expect(backingSize(100_000, 14, 2).width).toBe(MAX_WIDTH);
  });

  it("survives a zero-sized or absent measurement", () => {
    expect(backingSize(0, 0, 1)).toEqual({ width: STEP, height: 1 });
    expect(backingSize(Number.NaN, 14, Number.NaN).width).toBeGreaterThan(0);
  });
});
