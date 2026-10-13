/**
 * Moving the window by its own title bar.
 *
 * `data-tauri-drag-region` alone is not enough here. It hands the drag to the
 * webview's own handler, which starts the move on `mousedown` and then never
 * sees the `mouseup` — the drag is happening outside the webview by then — so
 * the second attempt finds the region still believing a drag is running and
 * does nothing. Asking the window to drag itself has no such state.
 *
 * Outside Tauri there is no window to move, and this does nothing rather than
 * throwing: the same title bar is drawn in a browser and in Playwright.
 */
const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Starts moving the window. Ignores anything but a plain left-button press. */
export function startWindowDrag(event: {
  button: number;
  detail: number;
  preventDefault: () => void;
}): void {
  if (!isTauri || event.button !== 0) return;
  // A double click is the maximise gesture, not the start of a move.
  if (event.detail > 1) return;
  event.preventDefault();
  void (async () => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().startDragging();
    } catch (failure) {
      console.warn("the window would not start dragging", failure);
    }
  })();
}

/** Zooms the window, which is what a double click on a title bar does. */
export function toggleWindowMaximise(): void {
  if (!isTauri) return;
  void (async () => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().toggleMaximize();
    } catch (failure) {
      console.warn("the window would not zoom", failure);
    }
  })();
}
