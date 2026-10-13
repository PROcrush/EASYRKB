import { describe, expect, it } from "vitest";

import { formatCount, formatMemory, formatPercent } from "./useDiagnostics";

describe("formatMemory", () => {
  it("switches to gigabytes rather than printing five digits", () => {
    expect(formatMemory(312)).toBe("312 MB");
    expect(formatMemory(1536)).toBe("1.5 GB");
  });

  it("shows a dash rather than a zero it cannot vouch for", () => {
    // Zero is a claim: it says the app is resident in no memory at all.
    expect(formatMemory(0)).toBe("—");
    expect(formatMemory(Number.NaN)).toBe("—");
  });
});

describe("formatCount", () => {
  it("prints a whole number, and a dash for what the platform will not say", () => {
    expect(formatCount(42)).toBe("42");
    expect(formatCount(41.6)).toBe("42");
    expect(formatCount(null)).toBe("—");
    expect(formatCount(Number.NaN)).toBe("—");
  });
});

describe("formatPercent", () => {
  it("rounds, and dashes what is unavailable", () => {
    expect(formatPercent(12.4)).toBe("12%");
    expect(formatPercent(0)).toBe("0%");
    expect(formatPercent(null)).toBe("—");
  });

  it("keeps a tenth below 10%, so a quiet app does not read as a broken one", () => {
    expect(formatPercent(0.15)).toBe("0.1%");
    expect(formatPercent(3.42)).toBe("3.4%");
    expect(formatPercent(9.96)).toBe("10.0%");
  });
});
