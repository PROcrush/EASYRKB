/** Windowing maths shared by the track table and the tree. */

export interface Window {
  /** First row index to render, including overscan. */
  start: number;
  /** One past the last row index to render. */
  end: number;
}

/**
 * Rows to render for a scroll position. Overscan is in rows, applied both ways,
 * and the window is clamped to the row count so we never request past the end.
 */
export function visibleWindow(
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number,
  rowCount: number,
  overscan = 8,
): Window {
  if (rowHeight <= 0 || rowCount <= 0) return { start: 0, end: 0 };
  const first = Math.floor(scrollTop / rowHeight);
  const visible = Math.ceil(viewportHeight / rowHeight);
  const start = Math.max(0, first - overscan);
  const end = Math.min(rowCount, first + visible + overscan);
  return { start, end: Math.max(start, end) };
}

/**
 * Coalesces page requests: dedupes against what is already in flight and caps
 * how many go out at once, so a fast flick cannot queue hundreds of fetches.
 */
export function planFetches(missing: readonly number[], inFlight: ReadonlySet<number>, maxPerTick = 4): number[] {
  const out: number[] = [];
  for (const page of missing) {
    if (inFlight.has(page)) continue;
    out.push(page);
    if (out.length >= maxPerTick) break;
  }
  return out;
}
