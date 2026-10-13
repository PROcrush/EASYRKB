import { isTyping } from "./shortcuts";
import { setHistoryMenu } from "@/ipc/client";

export type HistoryAction = "undo" | "redo";
const EVENT = "deck-edit-history";
const available = new Map<symbol, { undo: string | null; redo: string | null }>();
let library = { undo: null as string | null, redo: null as string | null };

function updateMenu(): void {
  const focused = [...available.values()].at(-1);
  const labels = {
    undo: focused?.undo ?? library.undo,
    redo: focused?.redo ?? library.redo,
  };
  const typing = isTyping(document.activeElement);
  void setHistoryMenu(typing ? null : labels.undo, typing ? null : labels.redo).catch(console.error);
}

// Focus decides whether the native menu describes WebKit text history or the
// app/deck stack, even when no deck editor is mounted.
if (typeof document !== "undefined") {
  document.addEventListener("focusin", updateMenu);
  document.addEventListener("focusout", updateMenu);
}

/** The library stack is the fallback when no deck editor owns history. */
export function setLibraryEditHistory(undo: string | null, redo: string | null): void {
  library = { undo, redo };
  updateMenu();
}

/** Whether a focused editor currently owns this history action. */
export function hasEditHistory(action: HistoryAction): boolean {
  return [...available.values()].some((entry) => entry[action] !== null);
}

/** Native Edit menu commands follow focus, just like typing shortcuts. */
export function runEditHistory(action: HistoryAction): void {
  if (isTyping(document.activeElement)) {
    document.execCommand(action);
    return;
  }
  window.dispatchEvent(new CustomEvent<HistoryAction>(EVENT, { detail: action }));
}

/** Only the deck with the keyboard subscribes. */
export function listenEditHistory(
  handle: (action: HistoryAction) => void,
  undo: string | null = null,
  redo: string | null = null,
): () => void {
  const token = Symbol("edit-history");
  available.set(token, { undo, redo });
  const listener = (event: Event) => handle((event as CustomEvent<HistoryAction>).detail);
  window.addEventListener(EVENT, listener);
  updateMenu();
  return () => {
    available.delete(token);
    window.removeEventListener(EVENT, listener);
    updateMenu();
  };
}
