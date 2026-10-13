import { useEffect, useRef, useState } from "react";

import { formatBpm } from "@/lib/format";
import { MAX_TEMPO, MIN_TEMPO } from "@/lib/sync";
import { usePreferencesContext } from "@/store/usePreferences";
import styles from "./TempoField.module.css";

export function TempoToggle({ bpmX100, baseBpmX100, onBpmChange, className }: {
  bpmX100: number;
  baseBpmX100: number;
  onBpmChange: (bpm: number) => void;
  className?: string | undefined;
}) {
  const { preferences, update } = usePreferencesContext();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const clickTimer = useRef<number | null>(null);

  useEffect(() => {
    if (editing) input.current?.select();
  }, [editing]);
  useEffect(() => () => {
    if (clickTimer.current !== null) window.clearTimeout(clickTimer.current);
  }, []);

  const valid = (value: string) => {
    const bpm = Number(value);
    return /^\d+(?:\.\d{1,2})?$/.test(value.trim()) && Number.isFinite(bpm)
      && baseBpmX100 > 0 && bpm >= baseBpmX100 / 100 * MIN_TEMPO
      && bpm <= baseBpmX100 / 100 * MAX_TEMPO;
  };
  const commit = () => {
    if (valid(draft)) onBpmChange(Number(draft));
    setEditing(false);
  };

  if (editing) return (
    <input
      ref={input}
      className={`${styles.tempoInput} ${className ?? ""}`}
      aria-label="Player tempo in BPM"
      inputMode="decimal"
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
      onKeyDown={(event) => {
        event.stopPropagation();
        if (event.key === "Escape") setEditing(false);
        else if (event.key === "Enter") event.currentTarget.blur();
      }}
    />
  );

  return (
    <button type="button" className={`${styles.tempoToggle} ${className ?? ""}`}
      aria-label="Toggle tempo slider"
      aria-expanded={preferences.view.tempoSlider}
      title="Double-click to enter BPM"
      data-testid="player-bpm"
      onClick={() => {
        if (clickTimer.current !== null) window.clearTimeout(clickTimer.current);
        clickTimer.current = window.setTimeout(() => {
          update("view", { tempoSlider: !preferences.view.tempoSlider });
          clickTimer.current = null;
        }, 250);
      }}
      onDoubleClick={() => {
        if (clickTimer.current !== null) window.clearTimeout(clickTimer.current);
        clickTimer.current = null;
        setDraft(formatBpm(bpmX100));
        setEditing(true);
      }}>
      {formatBpm(bpmX100)}
    </button>
  );
}
