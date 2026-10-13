import { describe, expect, it } from "vitest";

import { reasonFrom } from "./usePlayback";

describe("reasonFrom", () => {
  it("says which failure it was, rather than that there was one", () => {
    // Both of these reach the same catch, and they need different actions:
    // one means plug the drive in, the other means the device is gone.
    expect(reasonFrom({ kind: "notFound", message: "That track's file could not be found." }))
      .toBe("That track's file could not be found.");
    expect(reasonFrom({ kind: "internal", message: "The audio device could not be opened." }))
      .toBe("The audio device could not be opened.");
  });

  it("falls back rather than showing an empty alert", () => {
    // A rejection with nothing in it still has to say something.
    for (const nothing of [null, undefined, "boom", 7, {}, { message: "" }, { message: "  " }]) {
      expect(reasonFrom(nothing)).toBe("This track could not be played.");
    }
  });

  it("does not print an object at the user", () => {
    expect(reasonFrom({ message: { nested: true } })).toBe("This track could not be played.");
  });
});
