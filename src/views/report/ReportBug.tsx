import { useEffect, useState } from "react";
import { getBackend } from "@/ipc/client";
import { formatBugReportAttachment } from "@/lib/bugReport";
import { loadPreferences } from "@/lib/preferences";
import { startWindowDrag } from "@/lib/windowDrag";
import { useShowWindowWhenReady } from "@/lib/windowReady";
import styles from "./ReportBug.module.css";

export function ReportBug({ onClose, windowed = false }: { onClose: () => void; windowed?: boolean }) {
  const [description, setDescription] = useState("");
  const [include, setInclude] = useState(true);
  const [attachment, setAttachment] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    if (!include) { setAttachment(null); return; }
    let live = true;
    void getBackend().then(backend => backend.reportAttachment()).then(text => {
      if (live) { setAttachment(text); setError(""); }
    }).catch((e: unknown) => { if (live) setError(e instanceof Error ? e.message : String(e)); });
    return () => { live = false; };
  }, [include]);
  const body = <section className={styles.dialog} role="dialog" aria-label="Report bug" aria-modal={windowed ? undefined : true}>
    <header className={styles.title} onMouseDown={windowed ? startWindowDrag : undefined}>
      Report bug
      {windowed ? null : <button type="button" aria-label="Close report" onClick={onClose}>×</button>}
    </header>
    <form className={styles.form} onSubmit={event => {
      event.preventDefault();
      if (busy || !description.trim() || (include && attachment === null)) return;
      setBusy(true); setError(""); setSaved(false);
      const report = formatBugReportAttachment(include ? attachment ?? "" : "", description, loadPreferences());
      void getBackend().then(backend => backend.openReportAttachment(report))
        .then(() => setSaved(true))
        .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
        .finally(() => setBusy(false));
    }}>
      <div className={styles.fields}>
        <p className={styles.hint}>Reports stay on this Mac. Nothing is sent automatically.</p>
        <label className={styles.description}>What happened?
          <textarea required maxLength={30000} rows={7} placeholder="What were you doing, what went wrong, and what did you expect?" value={description} onChange={e => { setDescription(e.target.value); setSaved(false); }} />
        </label>
        <div className={styles.attachments}>
          <label className={styles.toggle}><input type="checkbox" checked={include} onChange={e => { setInclude(e.target.checked); setSaved(false); }} />Attach log and system information</label>
          <p className={styles.hint}>The log may include library paths and track titles.</p>
        </div>
        {saved ? <p role="status">Report saved locally and opened in your text editor.</p> : null}
        {error ? <p className={styles.error} role="alert">{error}</p> : null}
      </div>
      <footer>
        <button type="button" onClick={onClose}>Close</button>
        <button className={`${styles.save} ${styles.open}`} type="submit" disabled={busy || !description.trim() || (include && attachment === null)}>{busy ? "Saving…" : "Save and open report"}</button>
      </footer>
    </form>
  </section>;
  return windowed ? body : <div className={styles.backdrop}>{body}</div>;
}

export function ReportWindow() {
  useShowWindowWhenReady();
  return <ReportBug windowed onClose={() => { void getBackend().then(backend => backend.closeWindow()); }} />;
}
