import { describe, expect, it } from "vitest";
import { normalHeight, VuMeter } from "./vuMeter";

describe("Normal VU", () => {
  it("uses rekordbox's integer dB floor and 88-row threshold mapping", () => {
    expect(normalHeight(-44)).toBe(1 / 88);
    expect(normalHeight(-43)).toBe(2 / 88);
    expect(normalHeight(-24)).toBe(40 / 88);
    expect(normalHeight(-1)).toBe(86 / 88);
    expect(new VuMeter("normal").step(0.5, 0, 0).peak).toBe(74 / 88);
    expect(new VuMeter("normal").step(0.001, 0, 0).peak).toBe(0);
    expect(new VuMeter("normal").step(2, 0, 0).peak).toBe(1);
  });

  it("falls by whole dB steps after 20 ms, and keeps a separate two-second marker", () => {
    const meter = new VuMeter("normal");
    meter.step(1, 0, 0);
    expect(meter.step(0, 0, 20).peak).toBe(1);
    const next = meter.step(0, 0, 60);
    expect(next.peak).toBe(normalHeight(-3));
    expect(next.marker).toBe(1);
    meter.step(0, 0, 1000);
    expect(meter.step(0, 0, 100).peak).toBe(0);
    expect(meter.step(0, 0, 900).marker).toBe(0);
    expect(meter.active).toBe(false);
  });

  it("does not extend the marker timer on equal readings", () => {
    const meter = new VuMeter("normal");
    meter.step(1, 0, 0);
    meter.step(1, 0, 1000);
    meter.step(0, 0, 900);
    expect(meter.step(0, 0, 150).marker).toBe(0);
  });

  it("never releases below the current signal", () => {
    const meter = new VuMeter("normal");
    meter.step(1, 0, 0);
    expect(meter.step(10 ** (-2 / 20), 0, 100).peak).toBe(normalHeight(-2));
  });
});

describe("Fabulous VU", () => {
  it("shows independent audio RMS and peaks on a 32 dB scale", () => {
    const meter = new VuMeter("fabulous");
    const reading = meter.step(1, 1 / Math.sqrt(2), 0);
    expect(reading.peak).toBe(1);
    expect(reading.rms).toBeCloseTo((32 - 3.0103) / 32);
    expect(meter.step(1, 0, 60).rms).toBe(0);
  });

  it("holds for 50 ms then falls at 8 dB/s, with a slower held marker", () => {
    const meter = new VuMeter("fabulous");
    meter.step(1, 0.5, 0);
    expect(meter.step(0, 0, 50).peak).toBe(1);
    const reading = meter.step(0, 0, 1000);
    expect(reading.peak).toBe(24 / 32);
    expect(reading.marker).toBe(1);
    expect(reading.rms).toBe(0);
    const later = meter.step(0, 0, 1000);
    expect(later.peak).toBe(16 / 32);
    expect(later.marker).toBe(17 / 32);
    meter.step(0, 0, 5000);
    expect(meter.active).toBe(false);
  });

  it("handles invalid readings without poisoning the display", () => {
    for (const mode of ["normal", "fabulous"] as const) {
      const reading = new VuMeter(mode).step(NaN, Infinity, NaN);
      expect(reading).toEqual({ peak: 0, rms: 0, marker: 0 });
    }
  });
});
