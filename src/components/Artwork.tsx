/**
 * A track's sleeve, which shows nothing at all until it has one.
 *
 * An `<img>` whose source has not arrived is not empty: WebKit draws its own
 * broken-file glyph in it, and `alt=""` does not suppress that — only the alt
 * *text*. At startup every visible row asks for artwork before the library is
 * open, so the first screen was a grid of question marks over the hue tint.
 *
 * So the image is hidden until it has actually decoded, and the ground behind
 * it is left to the caller: a track with artwork shows an empty square while
 * it loads rather than a placeholder that is about to be replaced.
 *
 * The retry is what makes that safe. A request made before the library is open
 * fails, and an `<img>` never asks again on its own — the row would stay blank
 * for as long as it stayed on screen, which is worse than the glyph. A handful
 * of attempts covers the second the library takes to come up; past that the
 * file really is unreadable and the tint stands in for good.
 */
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

import { artworkUrl, attemptUrl, retryDelay } from "@/ipc/artwork";

export interface ArtworkProps {
  trackId: string;
  className?: string | undefined;
  /** Rows are virtualised, so their sleeves are worth deferring. */
  lazy?: boolean;
}

export function Artwork({ trackId, className, lazy = false }: ArtworkProps) {
  const ref = useRef<HTMLImageElement>(null);
  const [state, setState] = useState<"pending" | "shown" | "failed">("pending");
  /** Bumped to ask again, and part of the `src` so the browser really does. */
  const [attempt, setAttempt] = useState(0);
  const timer = useRef(0);

  const src = artworkUrl(trackId);

  // A recycled row is a different track: forget what the last one managed.
  //
  // During render, not in an effect. Layout effects run before passive ones,
  // so a reset in a passive effect landed *after* the cached-image check below
  // and knocked an already-decoded sleeve back to pending — and a cached image
  // never fires `load` again, so that square stayed empty for good.
  const [lastId, setLastId] = useState(trackId);
  if (lastId !== trackId) {
    setLastId(trackId);
    setState("pending");
    setAttempt(0);
  }

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    [],
  );

  // A cached image can be complete before React attaches its handlers, and
  // `load` never fires again — that sleeve would stay hidden for ever.
  useLayoutEffect(() => {
    const img = ref.current;
    if (img?.complete === true && img.naturalWidth > 0) setState("shown");
  }, [src, attempt]);

  const onError = useCallback(() => {
    setAttempt((at) => {
      const wait = retryDelay(at);
      if (wait === null) {
        // Out of attempts: the file really is unreadable, so let whatever
        // ground the caller draws stand in for it from here on.
        setState("failed");
        return at;
      }
      if (timer.current) clearTimeout(timer.current);
      timer.current = window.setTimeout(
        () => setAttempt((now) => (now === at ? at + 1 : now)),
        wait,
      );
      return at;
    });
  }, []);

  // Outside Tauri there is no scheme to fetch from, so there is no image.
  if (src === undefined) return null;

  return (
    <img
      ref={ref}
      className={className}
      src={attemptUrl(src, attempt)}
      alt=""
      // Read by CSS: hidden unless shown, and the ground behind it is only
      // drawn once this has given up rather than while it is still trying.
      data-state={state}
      draggable={false}
      decoding="async"
      {...(lazy ? { loading: "lazy" as const } : {})}
      onLoad={() => setState("shown")}
      onError={onError}
    />
  );
}
