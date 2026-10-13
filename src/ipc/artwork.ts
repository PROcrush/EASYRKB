/**
 * Where a track's artwork comes from.
 *
 * A custom scheme rather than `invoke`: a JPEG is tens of kilobytes, which
 * blows the 64 KB IPC cap, and a base64 payload would cost a main-thread
 * decode for every row. An `<img src>` lets the webview fetch, decode and
 * cache it off the UI thread.
 *
 * Outside Tauri — `pnpm dev:mock` and the Playwright suite — there is no such
 * scheme, so this returns nothing and the tint stands in.
 */
import { convertFileSrc } from "@tauri-apps/api/core";

const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function artworkUrl(trackId: string): string | undefined {
  if (!isTauri) return undefined;
  // The scheme's root as this platform's webview spells it: `rbl://localhost/`
  // on macOS and Linux, `http://rbl.localhost/` on Windows, where WebView2
  // does not load custom schemes and `rbl://` fails as an unknown scheme.
  // Both reach the handler as `rbl://localhost/…`.
  //
  // The backend resolves the id against the library; it never takes a path
  // from here, so nothing in the webview can name a file to read.
  return `${convertFileSrc("", "rbl")}artwork/${encodeURIComponent(trackId)}`;
}

/**
 * Attempts after the first, which is enough to outlast the library opening.
 *
 * A sleeve asked for before the library is open fails, and an `<img>` never
 * asks again on its own: the square would stay empty for as long as the row
 * stayed on screen. Three more tries cover the second the library takes.
 */
export const ARTWORK_RETRIES = 3;

/** The first wait, doubling on each attempt after it. */
export const ARTWORK_RETRY_MS = 250;

/** How long to wait before asking again, or null once there is no point. */
export function retryDelay(attempt: number): number | null {
  if (!Number.isInteger(attempt) || attempt < 0) return null;
  if (attempt >= ARTWORK_RETRIES) return null;
  return ARTWORK_RETRY_MS * 2 ** attempt;
}

/**
 * The source for one attempt.
 *
 * The attempt goes in the query because a browser will not re-request a URL it
 * has already failed on; the backend resolves the id and ignores the rest.
 */
export function attemptUrl(src: string, attempt: number): string {
  return attempt <= 0 ? src : `${src}?retry=${attempt}`;
}
