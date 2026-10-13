import { useCallback, useEffect, useRef, useState } from "react";
import { setConfirmHandler, type ConfirmLabels } from "@/ipc/confirmHost";
import { useTranslation } from "@/i18n";
import styles from "./ConfirmDialog.module.css";

interface Request {
  message: string;
  labels: ConfirmLabels | undefined;
  settle: (answer: boolean) => void;
}

/**
 * Asks the yes/no questions the backend's `confirm` is given, in the app's own
 * window instead of the operating system's. Mounted once; questions asked
 * while one is open wait their turn.
 */
export function ConfirmHost() {
  const t = useTranslation();
  const [queue, setQueue] = useState<readonly Request[]>([]);
  const no = useRef<HTMLButtonElement>(null);
  const current = queue[0];

  useEffect(() => setConfirmHandler((message, labels) => new Promise<boolean>((resolve) => {
    setQueue((waiting) => [...waiting, { message, labels, settle: resolve }]);
  })), []);

  const answer = useCallback((yes: boolean) => {
    current?.settle(yes);
    setQueue((waiting) => waiting.slice(1));
  }, [current]);

  // No is the focused button: a question about removing things should not be
  // answered yes by a stray Enter.
  useEffect(() => { if (current) no.current?.focus(); }, [current]);

  useEffect(() => {
    if (!current) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      answer(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [current, answer]);

  if (!current) return null;
  const title = current.labels?.title ?? "EASYRKB";
  return (
    <div className={styles.backdrop} role="presentation">
      <div className={styles.window} role="alertdialog" aria-modal="true" aria-label={title}
        aria-describedby="confirm-message">
        <div className={styles.title}>{title}</div>
        <p id="confirm-message" className={styles.message}>{current.message}</p>
        <div className={styles.buttons}>
          <button ref={no} type="button" className={styles.button} onClick={() => answer(false)}>
            {current.labels?.no ?? t("No")}
          </button>
          <button type="button" className={`${styles.button} ${styles.primary}`} onClick={() => answer(true)}>
            {current.labels?.yes ?? t("Yes")}
          </button>
        </div>
      </div>
    </div>
  );
}
