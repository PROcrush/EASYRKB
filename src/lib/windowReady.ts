import { useEffect } from "react";

/**
 * How long a window waits for its first data before it shows anyway. A slow
 * device scan or a library that is still loading must not leave the window
 * invisible; past this it opens with its loading text instead.
 */
const MAX_WAIT_MS = 1000;

let shown = false;

function showWindow(): void {
  if (shown || !("__TAURI_INTERNALS__" in window)) return;
  shown = true;
  void import("@tauri-apps/api/core")
    .then(({ invoke }) => invoke("show_window"))
    .catch(() => { /* A reveal failure must not break the web build. */ });
}

/**
 * Native windows are created hidden. Reveal this one once React has committed
 * its contents and `ready` is true — the window's first data has arrived —
 * so it never reaches the screen empty or half filled. After `MAX_WAIT_MS` it
 * is shown whether or not the data came.
 */
export function useShowWindowWhenReady(ready = true): void {
  useEffect(() => {
    if (ready) showWindow();
  }, [ready]);
  useEffect(() => {
    const timer = setTimeout(showWindow, MAX_WAIT_MS);
    return () => clearTimeout(timer);
  }, []);
}
