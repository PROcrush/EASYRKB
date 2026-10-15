import { useEffect, useRef, useState } from "react";
import { getBackend } from "@/ipc/client";
import { useTranslation } from "@/i18n";
import { errorMessage } from "@/lib/errorMessage";
import styles from "./NcmConverter.module.css";

type Item = { source: string; status: "pending" | "converting" | "success" | "failed"; output?: string; error?: string | undefined };
const filename = (path: string) => path.split(/[/\\]/).pop() ?? path;

export function NcmConverter({ onClose }: { onClose: () => void }) {
  const t = useTranslation();
  const dialog = useRef<HTMLDialogElement>(null);
  const [items, setItems] = useState<Item[]>([]);
  const [directory, setDirectory] = useState("");
  const [busy, setBusy] = useState(false);
  const [picking, setPicking] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [error, setError] = useState("");
  const stop = useRef(false);
  const running = useRef(false);
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    const previous = document.activeElement;
    dialog.current?.showModal();
    return () => { alive.current = false; stop.current = true; if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, []);
  const choose = async (kind: "files" | "directory") => {
    setPicking(true); setError("");
    try {
      const backend = await getBackend();
      if (kind === "files") {
        const paths = await backend.ncmSelectFiles();
        if (paths && alive.current) setItems(current => {
          const known = new Set(current.map(item => item.source));
          const added = paths.filter(path => {
            if (known.has(path) || !/\.ncm$/i.test(path)) return false;
            known.add(path); return true;
          });
          return [...current, ...added.map(source => ({ source, status: "pending" as const }))];
        });
      } else {
        const path = await backend.ncmSelectOutput();
        if (path && alive.current) setDirectory(path);
      }
    } catch (e) { if (alive.current) setError(errorMessage(e)); }
    finally { if (alive.current) setPicking(false); }
  };
  const convert = async () => {
    if (running.current || !directory) return;
    running.current = true; stop.current = false;
    setBusy(true); setStopping(false); setError("");
    try {
      const backend = await getBackend();
      for (const item of items) {
        if (item.status === "success") continue;
        if (stop.current || !alive.current) break;
        const update = (change: Partial<Item>) => { if (alive.current) setItems(current => current.map(row => row.source === item.source ? { ...row, ...change } : row)); };
        update({ status: "converting", error: undefined });
        try {
          const result = await backend.ncmConvertFile(item.source, directory);
          update({ status: "success", output: result.outputPath });
        } catch (e) { update({ status: "failed", error: errorMessage(e) }); }
      }
    } catch (e) { if (alive.current) setError(errorMessage(e)); }
    finally { running.current = false; if (alive.current) { setBusy(false); setStopping(false); } }
  };
  const openOutput = async () => {
    try { await (await getBackend()).ncmOpenOutput(directory); }
    catch (e) { if (alive.current) setError(errorMessage(e)); }
  };
  const succeeded = items.reduce((count, item) => count + Number(item.status === "success"), 0);
  const failed = items.reduce((count, item) => count + Number(item.status === "failed"), 0);
  const disabled = busy || picking;
  const labels = { pending: "Waiting", converting: "Converting…", success: "Converted", failed: "Conversion failed" };
  return <dialog ref={dialog} className={styles.dialog} aria-label={t("NCM format conversion")}
    onKeyDown={e => e.stopPropagation()} onCancel={e => { e.preventDefault(); if (!disabled) onClose(); }}>
    <header className={styles.header}>
      <div><span className={styles.badge}>NCM → MP3 / FLAC</span><h2>{t("NCM format conversion")}</h2></div>
      <button type="button" disabled={disabled} onClick={onClose}>{t("Close")}</button>
    </header>
    <p className={styles.description}>{t("Convert local NetEase Cloud Music NCM files to their original MP3 or FLAC audio without re-encoding. Everything is processed on this computer.")}</p>
    <div className={styles.controls}>
      <button type="button" disabled={disabled} onClick={() => { void choose("files"); }}>{t("Add NCM files")}</button>
      <button type="button" disabled={disabled || items.length === 0} onClick={() => setItems([])}>{t("Clear list")}</button>
      <span>{t("{count} files selected", { count: items.length })}</span>
    </div>
    <div className={styles.output}>
      <label htmlFor="ncm-output">{t("Output folder")}</label>
      <div><input id="ncm-output" readOnly value={directory} placeholder={t("Choose a folder for converted audio")} /><button type="button" disabled={disabled} onClick={() => { void choose("directory"); }}>{t("Choose output folder")}</button></div>
      <small>{t("Original NCM files are kept. Existing output files are never overwritten; duplicate names receive a numbered suffix.")}</small>
    </div>
    <div className={styles.queue}>
      {items.length ? <table><thead><tr><th>{t("Source file")}</th><th>{t("Status / output file")}</th></tr></thead>
        <tbody>{items.map(item => <tr key={item.source} data-status={item.status}>
          <td title={item.source} data-i18n-ignore>{filename(item.source)}</td>
          <td><span className={styles[item.status]}>{t(labels[item.status])}</span>
            {item.output ? <small title={item.output} data-i18n-ignore>{filename(item.output)}</small> : null}
            {item.error ? <small className={styles.failed}>{t(item.error)}</small> : null}</td>
        </tr>)}</tbody></table> : <div className={styles.empty}><strong>{t("Add NCM files to get started")}</strong><span>{t("Select one or more .ncm files, then choose where to save the audio.")}</span></div>}
    </div>
    {error ? <p role="alert" className={styles.failed}>{t(error)}</p> : null}
    <footer className={styles.footer}>
      <p role="status" aria-live="polite">{busy ? t(stopping ? "Stopping after the current file…" : "Converting…") : t("Ready")}
        {items.length ? ` · ${t("{done} converted · {failed} failed · {total} total", { done: succeeded, failed, total: items.length })}` : ""}</p>
      <div className={styles.actions}>
        <button type="button" disabled={disabled || !directory} onClick={() => { void openOutput(); }}>{t("Open output folder")}</button>
        {busy ? <button type="button" disabled={stopping} onClick={() => { stop.current = true; setStopping(true); }}>{t("Stop after current file")}</button>
          : <button type="button" className={styles.primary} disabled={picking || !directory || items.length === 0 || succeeded === items.length} onClick={() => { void convert(); }}>{t(failed ? "Retry unfinished files" : "Start conversion")}</button>}
      </div>
    </footer>
  </dialog>;
}
