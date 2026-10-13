import styles from "./TempoField.module.css";
import { transposeKey } from "@/lib/camelot";

export function KeyShift({ musicalKey, shift, disabled, onChange }: {
  musicalKey: string;
  shift: number;
  disabled: boolean;
  onChange: (shift: number) => void;
}) {
  return (
    <div className={styles.keys} role="group" aria-label="Key shift">
      <button type="button" className={styles.key} aria-label="Key down a semitone"
        disabled={disabled || shift <= -12} onClick={() => onChange(shift - 1)}>←</button>
      <button type="button" className={styles.keyReset}
        aria-label="Reset key shift" disabled={disabled || shift === 0} onClick={() => onChange(0)}>
        <span className={styles.musicalKey} data-shifted={shift !== 0 || undefined}>{transposeKey(musicalKey, shift)}</span>
        {shift !== 0 ? (
          <span className={styles.keyShift} data-testid="key-shift">{`${shift > 0 ? "+" : ""}${shift}`}</span>
        ) : null}
      </button>
      <button type="button" className={styles.key} aria-label="Key up a semitone"
        disabled={disabled || shift >= 12} onClick={() => onChange(shift + 1)}>→</button>
    </div>
  );
}
