/**
 * An element's size in CSS pixels, kept current as the window changes.
 *
 * A `ResizeObserver` rather than a window listener: the player's waveforms
 * change width when the tree splitter moves as well as when the window does,
 * and only the element itself sees both.
 *
 * The ref is a callback, so the observer follows the element rather than the
 * first one it was handed. The layout switch swaps the deck's overview for the
 * simple player's, and an observer bound once at mount kept watching the old
 * element — which, detached, reported 0x0, and the new strip's waveform was
 * drawn into a 32x2 canvas.
 */
import { useCallback, useRef, useState } from "react";

export interface Size {
  width: number;
  height: number;
}

export function useElementSize<T extends HTMLElement>(): [
  React.RefCallback<T>,
  Size,
] {
  const [size, setSize] = useState<Size>({ width: 0, height: 0 });
  const observer = useRef<ResizeObserver | null>(null);

  const ref = useCallback((element: T | null) => {
    observer.current?.disconnect();
    observer.current = null;
    if (!element) return;
    const watcher = new ResizeObserver(([entry]) => {
      if (!entry) return;
      const box = entry.contentRect;
      // Only a real change: an observer that sets state to an equal object on
      // every callback would rerender the player for nothing.
      setSize((current) =>
        current.width === box.width && current.height === box.height
          ? current
          : { width: box.width, height: box.height },
      );
    });
    watcher.observe(element);
    observer.current = watcher;
  }, []);

  return [ref, size];
}
