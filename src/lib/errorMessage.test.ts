import { describe, expect, it } from "vitest";
import { errorMessage } from "./errorMessage";

describe("errorMessage", () => {
  it("includes the developer detail returned with an internal error", () => {
    expect(errorMessage({
      kind: "internal",
      message: "Something went wrong inside EASYRKB.",
      detail: "The backup folder is not writable: permission denied",
    })).toBe("Something went wrong inside EASYRKB. The backup folder is not writable: permission denied");
  });

  it("does not repeat identical detail", () => {
    expect(errorMessage({ message: "Could not sync.", detail: "Could not sync." })).toBe("Could not sync.");
  });

  it("preserves ordinary errors and unknown fallbacks", () => {
    expect(errorMessage(new Error("Network unavailable"))).toBe("Network unavailable");
    expect(errorMessage(null)).toBe("Something went wrong. Please try again.");
  });
});
