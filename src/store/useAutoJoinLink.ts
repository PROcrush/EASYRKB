import { useEffect, useRef } from "react";

import type { LinkPeerSeen, LinkStatus } from "@/ipc/types";

/**
 * Starts LINK once when a player or mixer first becomes available.
 *
 * A person can still disconnect manually: the same visible devices do not
 * trigger another attempt until they leave and return. Likewise, a failed
 * start is not hammered repeatedly while the network remains unchanged.
 */
export function useAutoJoinLink(
  enabled: boolean,
  peers: readonly LinkPeerSeen[],
  status: LinkStatus | null,
  busy: boolean,
  start: () => Promise<unknown>,
): void {
  const attempted = useRef<string | null>(null);

  useEffect(() => {
    const available = peers
      .filter((peer) => peer.kind === "player" || peer.kind === "mixer")
      .map((peer) => `${peer.kind}:${peer.number}:${peer.address}`)
      .sort()
      .join("|");
    if (!enabled || available === "") {
      attempted.current = null;
      return;
    }
    if (status?.on) {
      attempted.current = available;
      return;
    }
    if (busy || status === null || status.problem || attempted.current === available) return;
    attempted.current = available;
    void start().catch(() => {
      // Status events and manual attempts surface errors. Wait for the
      // visible device set to change before making another automatic attempt.
    });
  }, [enabled, peers, status, busy, start]);
}
