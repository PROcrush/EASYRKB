import { useState } from "react";
import { createPortal } from "react-dom";
import { faderToTempo, TEMPO_RANGES, tempoRangeEnds, tempoToFader, type TempoRange } from "@/lib/player";
import styles from "./TempoField.module.css";

/** A CDJ fader: down increases tempo, and the midpoint is always unity. */
export function TempoSlider({ tempo, onTempo, idle, synced, masterTempo, onMasterTempo, onReset, resetLocked, onUnlock }: {
  tempo: number;
  onTempo: (tempo: number) => void;
  idle: boolean;
  synced: boolean;
  masterTempo: boolean;
  onMasterTempo: (on: boolean) => void;
  onReset: () => void;
  resetLocked: boolean;
  onUnlock: () => void;
}) {
  const [range, setRange] = useState<TempoRange>(6);
  const [cursor, setCursor] = useState<{ x: number; y: number; tempo: number } | null>(null);
  const ends = tempoRangeEnds(range);
  const disabled = idle || synced || resetLocked;
  const move = (event: React.PointerEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    const next = faderToTempo(2 * (event.clientY - rect.top) / Math.max(1, rect.height) - 1, range);
    onTempo(next);
    setCursor({ x: rect.left, y: event.clientY, tempo: next });
  };
  return (
    <aside className={styles.sideSlider} aria-label="Tempo slider">
      <button className={styles.range} aria-label="Tempo range"
        onClick={() => setRange(TEMPO_RANGES[(TEMPO_RANGES.indexOf(range) + 1) % TEMPO_RANGES.length]!)}>
        {range === "wide" ? "WIDE" : `±${range}`}
      </button>
      <button className={styles.range} aria-label="Master tempo" aria-pressed={masterTempo}
        data-on={masterTempo ? "" : undefined} disabled={idle} onClick={() => onMasterTempo(!masterTempo)}>MASTER TEMPO</button>
      <div className={styles.faderRow}>
        <button className={styles.resetLock} aria-label="Reset tempo" aria-pressed={resetLocked}
          data-on={resetLocked ? "" : undefined} disabled={idle}
          onClick={resetLocked ? onUnlock : onReset}>RESET</button>
        <div className={styles.track} role="slider" aria-label="Tempo" aria-orientation="vertical"
          aria-valuemin={-ends.down} aria-valuemax={ends.up} aria-valuenow={Number(((tempo - 1) * 100).toFixed(2))}
          aria-disabled={disabled} tabIndex={disabled ? -1 : 0}
          onPointerDown={(event) => {
            if (disabled || event.button !== 0) return;
            event.currentTarget.setPointerCapture(event.pointerId);
            move(event);
          }}
          onPointerMove={(event) => { if (!disabled && event.currentTarget.hasPointerCapture(event.pointerId)) move(event); }}
          onPointerUp={(event) => { setCursor(null); if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId); }}
          onLostPointerCapture={() => setCursor(null)}
          onPointerCancel={() => setCursor(null)}
          onKeyDown={(event) => {
            if (disabled) return;
            if (event.key === "Home") onTempo(1);
            else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
              const next = tempo + (event.key === "ArrowDown" ? 0.001 : -0.001);
              onTempo(Math.max(faderToTempo(-1, range), Math.min(faderToTempo(1, range), next)));
            } else return;
            event.preventDefault();
            event.stopPropagation();
          }}>
          <span className={styles.centre} aria-hidden />
          <span className={styles.knob} style={{ top: `${(tempoToFader(tempo, range) + 1) * 50}%` }} aria-hidden />
        </div>
      </div>
      {cursor && !disabled ? createPortal(
        <output className={styles.cursorReadout} style={{ left: cursor.x, top: cursor.y }}>
          {`${cursor.tempo >= 1 ? "+" : ""}${((cursor.tempo - 1) * 100).toFixed(2)}%`}
        </output>, document.body,
      ) : null}
    </aside>
  );
}
