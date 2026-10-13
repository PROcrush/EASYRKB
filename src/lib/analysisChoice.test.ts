// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import { lastAnalysisChoice, rememberAnalysisChoice } from "./analysisChoice";

describe("analysis choice memory", () => {
  beforeEach(() => localStorage.clear());

  it("starts empty and returns what was last confirmed", () => {
    expect(lastAnalysisChoice()).toEqual({});
    const choice = { mode: "rbxport", bpmGrid: false, key: true, highPrecision: false, minBpm: 98, maxBpm: 195, firstBeatCue: true } as const;
    rememberAnalysisChoice(choice);
    expect(lastAnalysisChoice()).toEqual(choice);
  });

  it("ignores a malformed entry", () => {
    localStorage.setItem("rbxport.analysisChoice", '{"bpmGrid":"yes","mode":"x"}');
    expect(lastAnalysisChoice()).toEqual({});
  });
});
