/**
 * Where the vocals are, as a thin strip above the overview waveform.
 *
 * rekordbox draws short blue dashes over the stretches of a track that carry
 * vocals — the thing you look at to find the drop or the last chorus. The data
 * is the `PVDI` tag in the `.2EX` analysis file, one value per overview column.
 *
 * Drawn as a canvas rather than as elements: a run of a thousand `<span>`s for
 * a five-pixel strip is a lot of layout for something this small.
 */
import { memo, useEffect, useRef, useState } from "react";
import theme from "@/styles/theme";

import { getBackend } from "@/ipc/client";
import { backingSize } from "@/lib/canvasSize";
import { useElementSize } from "@/store/useElementSize";
import styles from "./Player.module.css";

/** Bytes per track, shared so a re-render does not refetch. */
const byTrack = new Map<string, Uint8Array>();
const inFlight = new Map<string, Promise<Uint8Array>>();

async function load(trackId: string): Promise<Uint8Array> {
  const cached = byTrack.get(trackId);
  if (cached) return cached;
  const existing = inFlight.get(trackId);
  if (existing) return existing;

  const pending = (async () => {
    try {
      const backend = await getBackend();
      const data = await backend.trackVocals(trackId);
      byTrack.set(trackId, data);
      return data;
    } catch {
      // A track without the tag simply has no strip.
      return new Uint8Array();
    } finally {
      inFlight.delete(trackId);
    }
  })();
  inFlight.set(trackId, pending);
  return pending;
}

/**
 * Above which value a column counts as carrying a vocal.
 *
 * **Unverified.** The tag's value range has not been characterised against
 * rekordbox's own rendering, so this is a midpoint of a byte rather than a
 * measured threshold. It decides only how much of the strip is filled.
 */
const PRESENT = 128;

export const VocalStrip = memo(function VocalStrip({ trackId }: { trackId: string | null }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const [box, size] = useElementSize<HTMLDivElement>();
  const [data, setData] = useState<Uint8Array | null>(null);

  useEffect(() => {
    if (trackId === null) {
      setData(null);
      return;
    }
    let live = true;
    setData(null);
    void load(trackId).then((bytes) => {
      if (live) setData(bytes);
    });
    return () => {
      live = false;
    };
  }, [trackId]);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const { width: w, height: h } = backingSize(size.width, size.height);
    if (canvas.width !== w) canvas.width = w;
    if (canvas.height !== h) canvas.height = h;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, w, h);
    if (!data || data.length === 0) return;

    ctx.fillStyle =
      getComputedStyle(canvas).getPropertyValue("--c-vocal").trim() || theme.color.vocal.value;
    const step = data.length / w;
    for (let x = 0; x < w; x++) {
      // The loudest column in this pixel's span, so a short vocal phrase is
      // not lost when many columns share a pixel.
      let peak = 0;
      const first = Math.floor(x * step);
      const last = Math.max(first + 1, Math.floor((x + 1) * step));
      for (let i = first; i < last && i < data.length; i++) {
        peak = Math.max(peak, data[i] ?? 0);
      }
      if (peak >= PRESENT) ctx.fillRect(x, 0, 1, h);
    }
  }, [data, size.width, size.height]);

  return (
    <div ref={box} className={styles.vocal} aria-hidden data-testid="player-vocal">
      <canvas ref={ref} />
    </div>
  );
});
