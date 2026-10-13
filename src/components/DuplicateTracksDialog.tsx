import { useCallback, useEffect, useRef, useState } from "react";

import { useTranslation } from "@/i18n";
import type { DuplicateTracks } from "@/lib/preferences";
import styles from "./ConfirmDialog.module.css";

export type DuplicateChoice = Exclude<DuplicateTracks, "ask">;

export interface DuplicateAnswer {
  choice: DuplicateChoice;
  remember: boolean;
}

/**
 * Asks what to do with tracks a playlist already holds: leave them out or add
 * them again. Escape and Cancel stop the whole add.
 */
export function DuplicateTracksDialog({ playlist, duplicates, total, onAnswer }: {
  playlist: string;
  duplicates: number;
  total: number;
  onAnswer: (answer: DuplicateAnswer | null) => void;
}) {
  const t = useTranslation();
  const [remember, setRemember] = useState(false);
  const skip = useRef<HTMLButtonElement>(null);

  useEffect(() => skip.current?.focus(), []);

  const answer = useCallback((choice: DuplicateChoice) => onAnswer({ choice, remember }), [onAnswer, remember]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      onAnswer(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onAnswer]);

  const title = t("Duplicate tracks");
  const message = duplicates === total
    ? t("{count} of the selected tracks are already in {name}.", { count: duplicates, name: playlist })
    : t("{count} of the {total} selected tracks are already in {name}.", { count: duplicates, total, name: playlist });
  return (
    <div className={styles.backdrop} role="presentation">
      <div className={styles.window} role="alertdialog" aria-modal="true" aria-label={title}
        aria-describedby="duplicate-tracks-message">
        <div className={styles.title}>{title}</div>
        <p id="duplicate-tracks-message" className={styles.message}>{message}</p>
        <label className={styles.remember}>
          <input type="checkbox" checked={remember} onChange={(event) => setRemember(event.target.checked)} />
          <span>{t("Remember my choice")}</span>
        </label>
        <div className={styles.buttons}>
          <button type="button" className={styles.button} onClick={() => onAnswer(null)}>{t("Cancel")}</button>
          <button type="button" className={styles.button} onClick={() => answer("add")}>{t("Add duplicates")}</button>
          <button ref={skip} type="button" className={`${styles.button} ${styles.primary}`}
            onClick={() => answer("skip")}>{t("Skip duplicates")}</button>
        </div>
      </div>
    </div>
  );
}
