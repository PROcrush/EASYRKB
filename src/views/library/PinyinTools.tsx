import { useEffect, useRef, useState } from "react";
import { getBackend } from "@/ipc/client";
import type { PinyinReading, PinyinProgress } from "@/ipc/types";
import { useTranslation } from "@/i18n";
import { errorMessage } from "@/lib/errorMessage";
import styles from "./PinyinTools.module.css";

export function PinyinGenerator({ tracks }: { tracks?: readonly string[] }) {
  const t = useTranslation();
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<PinyinProgress | null>(null);
  const [result, setResult] = useState("");
  const [error, setError] = useState("");
  const life = useRef({ live: true });
  useEffect(() => { const current = life.current; current.live = true; return () => { current.live = false; }; }, []);
  const generate = async () => {
    setBusy(true); setResult(""); setError(""); setProgress(null);
    let stop: (() => void) | undefined;
    try {
      const backend = await getBackend();
      const jobId = crypto.randomUUID();
      stop = backend.onPinyinProgress(next => { if (life.current.live && next.jobId === jobId) setProgress(next); });
      const generated = await backend.pinyinGenerate(tracks ?? null, jobId);
      if (life.current.live) setResult(t("Indexed {processed} tracks; {chinese} contain Chinese names; {skipped} unavailable tracks skipped.", { ...generated }));
    } catch (e) { if (life.current.live) setError(errorMessage(e)); }
    finally { stop?.(); if (life.current.live) setBusy(false); }
  };
  return <div className={styles.tools}>
    <button type="button" disabled={busy || tracks?.length === 0} onClick={() => { void generate(); }}>{t(tracks ? "Generate pinyin for selected tracks" : "Generate pinyin for the entire collection")}</button>
    {busy ? <p role="status">{t("Generating pinyin search index")}{progress ? ` (${progress.done}/${progress.total})` : "…"}</p> : null}
    {result ? <p role="status">{result}</p> : null}
    {error ? <p role="alert">{error}</p> : null}
  </div>;
}

export function PinyinDialog({ tracks, correct, onClose }: { tracks: readonly string[]; correct: boolean; onClose: () => void }) {
  const t = useTranslation();
  const dialog = useRef<HTMLDialogElement>(null);
  const [reading, setReading] = useState<PinyinReading | null>(null);
  const [title, setTitle] = useState("");
  const [artist, setArtist] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [saved, setSaved] = useState(false);
  useEffect(() => {
    let live = true;
    const previous = document.activeElement;
    dialog.current?.showModal();
    if (correct && tracks[0]) void getBackend().then(backend => backend.pinyinReading(tracks[0]!)).then(next => {
      if (live) { setReading(next); setTitle(next.titlePinyin); setArtist(next.artistPinyin); }
    }).catch(e => { if (live) setError(errorMessage(e)); });
    return () => { live = false; if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, [correct, tracks]);
  const save = async (automatic = false) => {
    if (!reading) return;
    setBusy(true); setError(""); setSaved(false);
    try {
      const next = await (await getBackend()).pinyinSave(reading.id, reading.title, reading.artist, automatic ? "" : title, automatic ? "" : artist);
      setReading(next); setTitle(next.titlePinyin); setArtist(next.artistPinyin); setSaved(true);
    } catch (e) { setError(errorMessage(e)); }
    finally { setBusy(false); }
  };
  return <dialog ref={dialog} className={styles.dialog} aria-label={t(correct ? "Correct pronunciation" : "Chinese library search")} onKeyDown={e => e.stopPropagation()} onCancel={e => { e.preventDefault(); if (!busy) onClose(); }}>
    <header><h2>{t(correct ? "Correct pronunciation" : "Chinese library search")}</h2><button type="button" disabled={busy} onClick={onClose}>{t("Close")}</button></header>
    <p>{t("Pinyin search keeps the original Chinese titles and artists. It does not change audio tags or rekordbox metadata.")}</p>
    {correct && reading ? <form className={styles.tools} onSubmit={e => { e.preventDefault(); void save(); }}>
      <p data-i18n-ignore>{reading.title} — {reading.artist}</p>
      <label>{t("Title pronunciation")}<input disabled={busy} value={title} onChange={e => { setTitle(e.target.value); setSaved(false); }} placeholder={t("Pinyin syllables separated by spaces")} /></label>
      <label>{t("Artist pronunciation")}<input disabled={busy} value={artist} onChange={e => { setArtist(e.target.value); setSaved(false); }} placeholder={t("Pinyin syllables separated by spaces")} /></label>
      <p>{t("Use spaces between syllables and v for ü. Leave a field empty to use its automatic reading.")}</p>
      <div className={styles.actions}><button disabled={busy} type="submit">{t("Save pronunciation")}</button><button disabled={busy} type="button" onClick={() => { void save(true); }}>{t("Restore automatic pronunciation")}</button></div>
      {saved ? <p role="status">{t("Pronunciation saved. Search aliases have been updated.")}</p> : null}
      <p>{t("Title initials: {title} · Artist initials: {artist}", { title: reading.titleInitials, artist: reading.artistInitials })}</p>
    </form> : !correct ? <PinyinGenerator tracks={tracks} /> : null}
    {error ? <p role="alert">{error}</p> : null}
  </dialog>;
}
