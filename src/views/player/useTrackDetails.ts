/**
 * A loaded track's full record, for the INFO tab beside the deck.
 *
 * Fetched only while the tab is showing: the deck loads a track on every
 * double-click and the MEMORY tab is what it opens on, so a record nobody is
 * looking at would be one IPC call per load for nothing. While the tab is
 * showing it is fetched once per track and again whenever the backend says
 * the library changed, which is what an edit in the information panel
 * announces — so a title or colour changed there shows on the deck without a
 * reload. Same shape as `useTrackCues`, which does the same for the cues.
 */
import { useEffect, useState } from "react";

import type { TrackDetails } from "@/ipc/types";
import { getBackend } from "@/ipc/client";

export function useTrackDetails(trackId: string | null, wanted: boolean): TrackDetails | null {
  const [details, setDetails] = useState<TrackDetails | null>(null);

  useEffect(() => {
    if (!wanted || trackId === null) return;
    let live = true;
    let stop: (() => void) | undefined;

    const fetch = async () => {
      const backend = await getBackend();
      let found: TrackDetails;
      try {
        found = await backend.trackDetails(trackId);
      } catch {
        // The tab prints what the row carries and leaves the rest blank.
        return;
      }
      // The track may have changed while this was in flight.
      if (live && found.id === trackId) setDetails(found);
    };

    void (async () => {
      const backend = await getBackend();
      if (!live) return;
      // Subscribed before the first fetch lands, so an edit made in the gap
      // is not missed.
      stop = backend.onLibraryChanged(() => {
        void fetch();
      });
      await fetch();
    })();

    return () => {
      live = false;
      stop?.();
    };
  }, [trackId, wanted]);

  // A record is only ever the loaded track's: the previous track's stays in
  // state until the next one arrives, and is never shown.
  return details && details.id === trackId ? details : null;
}
