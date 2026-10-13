/**
 * The BPM the deck is playing at, and the two ways to change it by hand.
 *
 * A double-click turns the number into a box to type a BPM into; the tempo
 * becomes whatever makes the track play at that BPM. A drag on the number
 * opens the tempo fader a CDJ has — ±6, ±10, ±16 or WIDE, the range chosen
 * above it — and the same drag drives the fader: down is faster, as a CDJ's
 * is. The key controls live in the deck header.
 */
import { useCallback, useEffect, useRef, useState } from "react";

import { formatBpm } from "@/lib/format";
import {
  faderToTempo, isClick, TEMPO_RANGES, tempoForTypedBpm, tempoRangeEnds, tempoToFader, type TempoRange,
} from "@/lib/player";
import { useTooltip } from "@/store/usePreferences";
import styles from "./TempoField.module.css";

/** The fader's travel in pixels, and so how far a drag goes end to end. */
const TRAVEL_PX = 120;

export interface TempoFieldProps {
  /** The track's own BPM, ×100; 0 when there is no track or no grid. */
  trackBpmX100: number;
  /** A multiple of the track's own speed. */
  tempo: number;
  onTempo: (tempo: number) => void;
  /** Nothing loaded, or the tempo is the master's: the field is read-only. */
  disabled: boolean;
  /** Why it is read-only, for the tooltip. */
  disabledBecause?: string | undefined;
  /** The host's own class for the number, so each strip keeps its measured box. */
  fieldClassName?: string | undefined;
}

export function TempoField({
  trackBpmX100, tempo, onTempo, disabled, disabledBecause,
  fieldClassName,
}: TempoFieldProps) {
  const tip = useTooltip();
  const [editing, setEditing] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [range, setRange] = useState<TempoRange>(6);
  const box = useRef<HTMLDivElement>(null);
  /** The press that may become a drag of the fader. */
  const press = useRef<{ x: number; y: number; tempo: number; dragging: boolean } | null>(null);

  const shown = formatBpm(Math.round(trackBpmX100 * tempo));
  const percent = (tempo - 1) * 100;

  // Anything outside, or Escape, closes the fader; a menu that outlives the
  // thing it was opened on is worse than no menu.
  useEffect(() => {
    if (!open) return;
    const outside = (event: MouseEvent) => {
      if (!box.current?.contains(event.target as Node)) setOpen(false);
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("mousedown", outside, true);
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("mousedown", outside, true);
      window.removeEventListener("keydown", key);
    };
  }, [open]);

  const commitTyped = useCallback(() => {
    if (editing === null) return;
    const next = tempoForTypedBpm(editing, trackBpmX100);
    setEditing(null);
    if (next !== null) onTempo(next);
  }, [editing, trackBpmX100, onTempo]);

  /** Where a vertical drag from `press` puts the fader: down is faster. */
  const dragTo = useCallback(
    (clientY: number) => {
      const held = press.current;
      if (!held) return;
      const at = tempoToFader(held.tempo, range) + ((clientY - held.y) / TRAVEL_PX) * 2;
      onTempo(faderToTempo(at, range));
    },
    [range, onTempo],
  );

  const onPointerDown = (event: React.PointerEvent<HTMLElement>) => {
    if (disabled || editing !== null || event.button !== 0) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    press.current = { x: event.clientX, y: event.clientY, tempo, dragging: false };
  };
  const onPointerMove = (event: React.PointerEvent<HTMLElement>) => {
    const held = press.current;
    if (!held || !event.currentTarget.hasPointerCapture(event.pointerId)) return;
    if (!held.dragging) {
      if (isClick(event.clientX - held.x, event.clientY - held.y)) return;
      held.dragging = true;
      setOpen(true);
    }
    dragTo(event.clientY);
  };
  const onPointerUp = (event: React.PointerEvent<HTMLElement>) => {
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    press.current = null;
  };

  /** A press on the fader itself: jump there, then drag. */
  const onFaderDown = (event: React.PointerEvent<HTMLDivElement>) => {
    if (disabled) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    const track = event.currentTarget.getBoundingClientRect();
    const at = ((event.clientY - track.top) / Math.max(track.height, 1)) * 2 - 1;
    onTempo(faderToTempo(at, range));
    press.current = { x: event.clientX, y: event.clientY, tempo: faderToTempo(at, range), dragging: true };
  };

  const ends = tempoRangeEnds(range);
  const knobAt = (tempoToFader(tempo, range) + 1) / 2;

  return (
    <div ref={box} className={styles.field}>
      {editing !== null ? (
        <>
          {/* Keeps the editor exactly as wide and tall as the readout it
              replaced; an input's default 20-character width is much wider. */}
          <span
            className={fieldClassName ? `${fieldClassName} ${styles.bpm} ${styles.sizer}` : `${styles.bpm} ${styles.sizer}`}
            aria-hidden
          >
            {shown}
          </span>
          <input
            className={fieldClassName ? `${fieldClassName} ${styles.input}` : styles.input}
            aria-label="BPM"
            value={editing}
            autoFocus
            inputMode="decimal"
            onChange={(event) => setEditing(event.target.value)}
            onBlur={commitTyped}
            onKeyDown={(event) => {
              if (event.key === "Enter") commitTyped();
              else if (event.key === "Escape") setEditing(null);
              event.stopPropagation();
            }}
          />
        </>
      ) : (
        <span
          className={fieldClassName ? `${fieldClassName} ${styles.bpm}` : styles.bpm}
          data-testid="player-bpm"
          // A number that can be adjusted, not a button: the capture's rows
          // count their buttons, and rekordbox's BPM field is not one.
          role="spinbutton"
          aria-valuenow={Number(shown)}
          tabIndex={disabled ? -1 : 0}
          aria-label="BPM"
          aria-disabled={disabled || undefined}
          aria-haspopup="dialog"
          aria-expanded={open}
          title={tip(disabled ? disabledBecause : "Double-click to type a BPM; drag for the tempo fader")}
          onDoubleClick={() => {
            if (!disabled && trackBpmX100 > 0) setEditing(shown);
          }}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
          onKeyDown={(event) => {
            if (disabled) return;
            if (event.key === "Enter") setEditing(shown);
            else if (event.key === "ArrowDown") setOpen(true);
          }}
        >
          {shown}
        </span>
      )}

      {open ? (
        <div className={styles.fader} role="dialog" aria-label="Tempo">
          <div className={styles.ranges} role="radiogroup" aria-label="Tempo range">
            {TEMPO_RANGES.map((choice) => (
              <button
                key={String(choice)}
                type="button"
                role="radio"
                aria-checked={range === choice}
                className={styles.range}
                data-on={range === choice ? "" : undefined}
                onClick={() => setRange(choice)}
              >
                {choice === "wide" ? "WIDE" : `±${choice}`}
              </button>
            ))}
          </div>
          <div className={styles.readout} data-testid="tempo-percent">
            {`${percent >= 0 ? "+" : "−"}${Math.abs(percent).toFixed(2)}%`}
          </div>
          <div
            className={styles.track}
            role="slider"
            aria-label="Tempo"
            aria-valuemin={-ends.down}
            aria-valuemax={ends.up}
            aria-valuenow={Number(percent.toFixed(2))}
            aria-orientation="vertical"
            tabIndex={0}
            onPointerDown={onFaderDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerCancel={onPointerUp}
            onKeyDown={(event) => {
              // A CDJ's fader: down is faster. A tenth of a percent a press.
              if (event.key === "ArrowDown") onTempo(Math.min(tempo + 0.001, faderToTempo(1, range)));
              else if (event.key === "ArrowUp") onTempo(Math.max(tempo - 0.001, faderToTempo(-1, range)));
              else return;
              event.preventDefault();
            }}
          >
            <span className={styles.centre} aria-hidden />
            <span className={styles.knob} style={{ top: `${knobAt * 100}%` }} aria-hidden />
            <span className={styles.end} data-end="top" aria-hidden>{`−${ends.down}`}</span>
            <span className={styles.end} data-end="bottom" aria-hidden>{`+${ends.up}`}</span>
          </div>

        </div>
      ) : null}
    </div>
  );
}
