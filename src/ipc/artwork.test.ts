/**
 * @vitest-environment jsdom
 */
import { afterEach, describe, expect, it, vi } from "vitest";

import { ARTWORK_RETRIES, ARTWORK_RETRY_MS, attemptUrl, retryDelay } from "./artwork";

describe("retryDelay", () => {
  it("backs off, doubling each time", () => {
    expect(retryDelay(0)).toBe(ARTWORK_RETRY_MS);
    expect(retryDelay(1)).toBe(ARTWORK_RETRY_MS * 2);
    expect(retryDelay(2)).toBe(ARTWORK_RETRY_MS * 4);
  });

  it("gives up rather than asking for ever", () => {
    // A file that is genuinely unreadable must stop costing requests, and the
    // tint behind it must be allowed to stand in.
    expect(retryDelay(ARTWORK_RETRIES)).toBeNull();
    expect(retryDelay(ARTWORK_RETRIES + 5)).toBeNull();
  });

  it("takes nothing but a whole count of attempts", () => {
    for (const bad of [-1, 0.5, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(retryDelay(bad)).toBeNull();
    }
  });

  it("covers the second the library takes to open", () => {
    let total = 0;
    for (let at = 0; at < ARTWORK_RETRIES; at++) total += retryDelay(at) ?? 0;
    expect(total).toBeGreaterThanOrEqual(1000);
  });
});

describe("attemptUrl", () => {
  it("asks for the plain source first", () => {
    expect(attemptUrl("rbl://localhost/artwork/7", 0)).toBe("rbl://localhost/artwork/7");
  });

  it("makes each retry a source the browser has not already failed on", () => {
    expect(attemptUrl("rbl://localhost/artwork/7", 1)).toBe("rbl://localhost/artwork/7?retry=1");
    expect(attemptUrl("rbl://localhost/artwork/7", 2)).toBe("rbl://localhost/artwork/7?retry=2");
  });
});

describe("artworkUrl", () => {
  const internals = window as unknown as Record<string, unknown>;
  afterEach(() => {
    delete internals["__TAURI_INTERNALS__"];
    vi.resetModules();
  });

  /** `artworkUrl` from a fresh module, under Tauri's `convertFileSrc` as one platform answers it. */
  async function underTauri(convertFileSrc: (path: string, protocol: string) => string) {
    internals["__TAURI_INTERNALS__"] = { convertFileSrc };
    vi.resetModules();
    return (await import("./artwork")).artworkUrl;
  }

  it("is the rbl scheme on macOS and Linux", async () => {
    const artworkUrl = await underTauri((path, protocol) => `${protocol}://localhost/${encodeURIComponent(path)}`);
    expect(artworkUrl("7")).toBe("rbl://localhost/artwork/7");
  });

  it("is the scheme's localhost host on Windows, where WebView2 loads no custom scheme", async () => {
    const artworkUrl = await underTauri((path, protocol) => `http://${protocol}.localhost/${encodeURIComponent(path)}`);
    expect(artworkUrl("7")).toBe("http://rbl.localhost/artwork/7");
  });

  it("escapes the id rather than letting it add to the path", async () => {
    const artworkUrl = await underTauri((path, protocol) => `${protocol}://localhost/${encodeURIComponent(path)}`);
    expect(artworkUrl("1/../2")).toBe("rbl://localhost/artwork/1%2F..%2F2");
  });

  it("is nothing outside Tauri", async () => {
    vi.resetModules();
    const { artworkUrl } = await import("./artwork");
    expect(artworkUrl("7")).toBeUndefined();
  });
});
