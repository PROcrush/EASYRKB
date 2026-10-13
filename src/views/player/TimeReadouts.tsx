import { memo, useCallback, useSyncExternalStore } from "react";
import type { Playback } from "@/store/usePlayback";
import { splitTime } from "@/lib/player";

export type PositionSource = Pick<Playback, "positionRef" | "subscribe">;

/** Only the time labels subscribe to tenths, not the containing deck. */
export const TimeReadouts = memo(function TimeReadouts({ source, total, classes, testId = "player-time" }: {
  source: PositionSource;
  total: number;
  classes: Readonly<Record<string, string>>;
  testId?: string;
}) {
  const snapshot = useCallback(() => Math.floor(source.positionRef.current * 10), [source.positionRef]);
  const tenths = useSyncExternalStore(source.subscribe, snapshot);
  const remaining = splitTenths(Math.max(Math.floor(total * 10) - tenths, 0));
  const elapsed = splitTenths(tenths);
  return <>
    <span className={classes.remaining} data-testid={testId}>
      -{remaining.main}<i className={classes.tenths}>.{remaining.tenths}</i>
    </span>
    <span className={classes.elapsed}>
      {elapsed.main}<i className={classes.tenths}>.{elapsed.tenths}</i>
    </span>
  </>;
});

function splitTenths(tenths: number) {
  return { main: splitTime(Math.floor(tenths / 10)).main, tenths: String(tenths % 10) };
}
