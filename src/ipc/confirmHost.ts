/**
 * Where a yes/no question is asked. The app shell registers its own dialog
 * here; until it does (a test, the first frame) the question falls back to
 * the operating system's dialog.
 */
export interface ConfirmLabels {
  yes: string;
  no: string;
  title?: string;
}

export type ConfirmHandler = (message: string, labels?: ConfirmLabels) => Promise<boolean>;

let handler: ConfirmHandler | null = null;

/** Returns the function that unregisters it. */
export function setConfirmHandler(next: ConfirmHandler): () => void {
  handler = next;
  return () => { if (handler === next) handler = null; };
}

export function confirmHandler(): ConfirmHandler | null {
  return handler;
}
