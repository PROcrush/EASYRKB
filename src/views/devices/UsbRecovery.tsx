import { useEffect, useRef, useState } from "react";
import { getBackend } from "@/ipc/client";
import type { Backend, Device, UsbRecoveryDiagnosis, UsbRecoveryExecution, UsbRecoveryPreview, UsbRecoveryProgress } from "@/ipc/types";
import { useTranslation } from "@/i18n";
import { errorMessage } from "@/lib/errorMessage";
import { usePreferences } from "@/store/usePreferences";
import { UsbRecoveryCache } from "./UsbRecoveryCache";
import styles from "./UsbRecovery.module.css";

const ISSUE_LABELS: Record<string, string> = {
  cacheLimit: "Diagnosis cache limit reached. Increase the limit and diagnose again.",
  sidecar: "Database sidecar copied to the local snapshot",
  databaseStructure: "Database structure is incomplete",
  unreadable: "Database could not be read",
  invalidRecord: "Invalid track record excluded",
  unsafePath: "Unsafe path excluded",
  missingAudio: "Audio could not be read",
  analysisUnreadable: "Analysis file could not be read",
  analysisMissing: "Analysis file is missing",
  artworkMissing: "Artwork is missing",
  playlistMembersMissing: "Unavailable playlist entries excluded",
  playlistHierarchy: "Invalid playlist hierarchy moved to the root",
};
const STAGES: Record<string, string> = { backup: "Backing up databases before repair", scanning: "Checking the source", checking: "Checking the recovery plan", copying: "Copying recovered tracks", database: "Writing device libraries", verifying: "Verifying recovery output", publishing: "Publishing recovery output", xml: "Writing recovery XML", complete: "Recovery completed" };
const STATUS = { readable: "Readable", partial: "Partially readable", unreadable: "Unreadable" };

/** Independent of DevicePanel: mounting this never writes device defaults. */
export function UsbRecoveryPane({ initialPath = "", onBusyChange }: { initialPath?: string; onBusyChange?: (busy: boolean) => void }) {
  const t = useTranslation();
  const preferences = usePreferences();
  const [path, setPath] = useState(initialPath);
  const [devices, setDevices] = useState<Device[]>([]);
  const [diagnosis, setDiagnosis] = useState<UsbRecoveryDiagnosis | null>(null);
  const [sourceId, setSourceId] = useState("");
  const [preview, setPreview] = useState<UsbRecoveryPreview | null>(null);
  const [kind, setKind] = useState<"repair" | "usb" | "xml">("repair");
  const [acknowledged, setAcknowledged] = useState(false);
  const [result, setResult] = useState<UsbRecoveryExecution | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<UsbRecoveryProgress | null>(null);
  const life = useRef<{ live: boolean; backend?: Backend; job: string | undefined; cancelled: boolean; report: string | undefined; previewRequest: number }>({ live: true, job: undefined, cancelled: false, report: undefined, previewRequest: 0 });
  const selected = diagnosis?.report.sources.find(s => s.id === sourceId);
  const repairFile = selected?.database.replace(/(?:export\.pdb|exportLibrary\.db)$/, selected.format === "Device Library" ? "exportLibrary.db" : "export.pdb");

  useEffect(() => { onBusyChange?.(busy); }, [busy, onBusyChange]);
  useEffect(() => {
    const current = life.current;
    current.live = true;
    let stop: (() => void) | undefined;
    let stopCache: (() => void) | undefined;
    void getBackend().then(async backend => {
      if (!current.live) return;
      current.backend = backend;
      stop = backend.onUsbRecoveryProgress(p => { if (current.live && p.jobId === current.job) setProgress(p); });
      stopCache = backend.onUsbRecoveryCacheChange(cleared => {
        if (!current.live || !cleared) return;
        current.report = undefined; current.previewRequest++;
        setDiagnosis(null); setPreview(null); setSourceId(""); setResult(null); setAcknowledged(false);
      });
      const connected = await backend.listDevices();
      if (current.live) setDevices(connected);
    }).catch(e => { if (current.live) setError(errorMessage(e)); });
    return () => {
      current.live = false;
      current.cancelled = true;
      stop?.();
      stopCache?.();
      if (current.job) void current.backend?.usbRecoveryCancel(current.job);
      if (current.report) void current.backend?.usbRecoveryRelease(current.report);
    };
  }, []);

  const reset = (next: string) => {
    if (life.current.job) return;
    if (life.current.report) void life.current.backend?.usbRecoveryRelease(life.current.report);
    life.current.report = undefined;
    life.current.previewRequest++;
    setPath(next); setDiagnosis(null); setPreview(null); setSourceId(""); setResult(null); setAcknowledged(false); setError("");
  };
  const scan = async () => {
    if (!path.trim() || life.current.job) return;
    reset(path);
    const current = life.current;
    const jobId = crypto.randomUUID(); current.job = jobId; current.cancelled = false;
    setBusy(true); setProgress(null);
    try {
      const backend = await getBackend(); current.backend = backend;
      if (!current.live || current.cancelled) return;
      const response = await backend.usbRecoveryDiagnose(path, jobId, preferences.usbExport.diagnosisCacheGiB);
      if (!current.live || current.cancelled) { await backend.usbRecoveryRelease(response.reportId); return; }
      current.report = response.reportId;
      setDiagnosis(response);
      // The user chooses the database; counts do not silently decide which version wins.
    } catch (e) { if (current.live) setError(errorMessage(e)); }
    finally { current.job = undefined; if (current.live) setBusy(false); }
  };
  const loadPreview = async (id: string, offset = 0) => {
    if (!diagnosis || busy) return;
    const request = ++life.current.previewRequest;
    setSourceId(id); setPreview(null); setAcknowledged(false); setResult(null); setError("");
    try {
      const response = await (await getBackend()).usbRecoveryPreview(diagnosis.reportId, id, offset);
      if (life.current.live && request === life.current.previewRequest) setPreview(response);
    } catch (e) { if (life.current.live && request === life.current.previewRequest) setError(errorMessage(e)); }
  };
  const execute = async () => {
    if (!diagnosis || !preview || !selected || !acknowledged || life.current.job) return;
    const current = life.current;
    const jobId = crypto.randomUUID(); current.job = jobId; current.cancelled = false;
    setBusy(true); setError(""); setProgress(null); setResult(null);
    try {
      const backend = await getBackend(); current.backend = backend;
      if (!current.live || current.cancelled) return;
      const destination = kind === "repair" ? diagnosis.report.sourcePath : kind === "xml" ? await backend.usbRecoveryXmlPath() : await backend.pickFolder("Choose an empty recovery drive or folder");
      if (!destination || !current.live || current.cancelled) return;
      const response = await backend.usbRecoveryExecute(diagnosis.reportId, sourceId, jobId, kind, destination);
      if (current.live) { setResult(response); setAcknowledged(false); }
    } catch (e) { if (current.live) setError(errorMessage(e)); }
    finally { current.job = undefined; if (current.live) setBusy(false); }
  };
  const pickSource = async () => {
    try { const next = await (await getBackend()).pickFolder("Choose a source USB drive or backup folder"); if (next && life.current.live) reset(next); }
    catch (e) { if (life.current.live) setError(errorMessage(e)); }
  };
  return <div className={styles.pane} aria-label={t("USB diagnosis and recovery")}>
    <p>{t("Diagnose a USB drive, back up and repair its database, or recover playlists and tracks to a new drive or XML.")}</p>
    <UsbRecoveryCache busy={busy} />
    <fieldset disabled={busy}>
      <legend>{t("1. Check the source")}</legend>
      <div className={styles.actions}>
        <select aria-label={t("Connected source drive")} value={devices.some(d => d.path === path) ? path : ""} onChange={e => reset(e.target.value)}>
          <option value="">{t("Select a connected USB drive")}</option>
          {devices.map(d => <option key={d.path} value={d.path} data-i18n-ignore>{d.name} — {d.path}</option>)}
        </select>
        <button type="button" onClick={() => { void pickSource(); }}>{t("Choose source folder…")}</button>
      </div>
      <label>{t("Source path")}<input value={path} onChange={e => reset(e.target.value)} placeholder={t("USB drive root or backup folder")} /></label>
      <button type="button" disabled={!path.trim()} onClick={() => { void scan(); }}>{t("Start read-only diagnosis")}</button>
    </fieldset>
    {diagnosis ? <fieldset disabled={busy}>
      <legend>{t("2. Choose a recovery source")}</legend>
      {diagnosis.report.pendingPublication ? <p className={styles.warning}>{t("An interrupted publication was found. Original files were left untouched; check the source versions before recovering.")}</p> : null}
      {diagnosis.report.sourceDisagreement ? <p className={styles.warning}>{t("The databases contain different versions. Choose the source you want to recover; they will not be merged automatically.")}</p> : null}
      {diagnosis.report.sources.length === 0 ? <p role="status">{t("No supported rekordbox device database was found. Select the root folder containing PIONEER or .PIONEER.")}</p> : null}
      {diagnosis.report.sources.map(source => <div className={styles.source} key={source.id}>
        <label className={styles.choice}><input type="radio" name="usb-recovery-source" disabled={source.status === "unreadable" || source.tracks === 0} checked={sourceId === source.id} onChange={() => { void loadPreview(source.id); }} />
          <strong data-i18n-ignore>{source.format}</strong><span>{t(STATUS[source.status])}</span>
        </label>
        <code data-i18n-ignore>{source.database}</code>
        <p>{t("{tracks} tracks · {playlists} playlists · {missing} missing audio files", { tracks: source.tracks, playlists: source.playlists, missing: source.missingAudio })}</p>
        <p>{t("Cues: {cues} tracks · Beat grids: {grids} tracks", { cues: source.cueTracks, grids: source.beatGridTracks })}</p>
        {source.issues.length ? <details><summary>{t("Diagnosis details")}</summary><ul>{source.issues.map((issue, i) => <li key={i}>{t(ISSUE_LABELS[issue.code] ?? "Diagnosis details")}: <code data-i18n-ignore>{issue.detail}</code></li>)}</ul></details> : null}
      </div>)}
      <button type="button" onClick={() => { void (async () => { try { await (await getBackend()).usbRecoveryOpenReport(diagnosis.reportId); } catch (e) { setError(errorMessage(e)); } })(); }}>{t("Open local recovery report")}</button>
    </fieldset> : null}
    {diagnosis && preview && selected ? <fieldset disabled={busy}>
      <legend>{t("3. Preview and recover")}</legend>
      <div className={styles.tableWrap}><table><caption>{t("Recovered track preview")}</caption><thead><tr><th>{t("Track Title")}</th><th>{t("Audio")}</th><th>{t("Cues")}</th><th>{t("Beat Grid")}</th></tr></thead><tbody>
        {preview.tracks.map(track => <tr key={track.id}><td data-i18n-ignore title={track.path}>{track.title}</td><td>{t(track.audioPresent ? "Available" : "Missing")}</td><td>{track.cues}</td><td>{track.beats}</td></tr>)}
      </tbody></table></div>
      <div className={styles.actions}><button type="button" disabled={preview.offset === 0} onClick={() => { void loadPreview(sourceId, Math.max(0, preview.offset - 100)); }}>{t("Previous")}</button>
        <span>{preview.offset + 1}–{Math.min(preview.total, preview.offset + preview.tracks.length)} / {preview.total}</span>
        <button type="button" disabled={preview.offset + preview.tracks.length >= preview.total} onClick={() => { void loadPreview(sourceId, preview.offset + 100); }}>{t("Next")}</button></div>
      <details><summary>{t("Recovered playlists and folders")}</summary><ul>{preview.playlists.map(p => <li key={p.id} data-i18n-ignore>{p.folder ? "▸ " : "♫ "}{p.name} ({p.entries})</li>)}</ul>{preview.playlists.length === 1000 ? <p>{t("The preview shows the first 1,000 playlist nodes; recovery includes all nodes.")}</p> : null}</details>
      <label>{t("Recovery output")}<select value={kind} onChange={e => { setKind(e.target.value === "xml" ? "xml" : e.target.value === "usb" ? "usb" : "repair"); setAcknowledged(false); }}>
        <option value="repair" disabled={diagnosis.report.pendingPublication}>{t("Repair the current USB drive")}</option><option value="usb">{t("Rebuild on a new empty USB drive")}</option><option value="xml">{t("Export recovery XML for rekordbox")}</option></select></label>
      <p>{t(kind === "repair" ? "Repair rebuilds the other device database from the selected source. A verified backup is saved on this computer first. Music files, analysis files and the selected database stay in place." : kind === "usb" ? "Both device libraries will be rebuilt. Existing audio and readable analysis files are preserved. The destination must be empty." : "XML includes playlists, cue points, loops and beat grids. Missing audio paths are retained for relocation in rekordbox. XML cannot preserve every player-specific analysis field.")}</p>
      {kind === "repair" ? <><p>{t("Database to rebuild")}: <code data-i18n-ignore>{repairFile}</code></p>{selected.format === "Device Library" ? <p>{t("Old WAL, SHM and journal files for this database are backed up and removed during replacement.")}</p> : null}</> : null}
      {selected.missingAudio > 0 ? <p className={styles.warning}>{t(kind === "repair" ? "{count} audio files are missing. Database repair keeps their records but cannot recreate the audio." : "{count} audio files are missing. New-drive recovery skips them; XML keeps their records and original paths.", { count: selected.missingAudio })}</p> : null}
      <p>{t(kind === "repair" ? "The rebuilt database includes the supported track and playlist fields. My Tags, settings and history in the selected database are kept; unavailable fields in the rebuilt database cannot be recreated." : "My Tags, device settings and play history are not restored in this version.")}</p>
      <label className={styles.choice}><input type="checkbox" checked={acknowledged} onChange={e => setAcknowledged(e.target.checked)} />{t("I have reviewed the source, missing files and recovery output.")}</label>
      <button type="button" className={styles.primary} disabled={!acknowledged || (kind === "repair" && diagnosis.report.pendingPublication) || (kind === "usb" && selected.tracks === selected.missingAudio)} onClick={() => { void execute(); }}>{t(kind === "repair" ? "Back up and repair this USB drive" : "Choose destination and start recovery…")}</button>
    </fieldset> : null}
    {busy ? <div role="status" aria-live="polite"><p>{t(STAGES[progress?.stage ?? ""] ?? "Preparing recovery")}{progress && progress.total > 0 ? ` (${progress.done}/${progress.total})` : ""}</p>
      {progress?.stage === "publishing" ? <p>{t("Finishing publication. Wait before unplugging the destination drive.")}</p> : <button type="button" onClick={() => { life.current.cancelled = true; if (life.current.job) void life.current.backend?.usbRecoveryCancel(life.current.job); }}>{t("Cancel recovery")}</button>}
    </div> : null}
    {result ? <div role="status" className={styles.result}><strong>{t(result.outcome.kind === "repair" ? "USB repair completed" : "Recovery completed")}</strong><p>{t("Saved {tracks} tracks and {playlists} playlists. Missing audio: {missing}.", { tracks: result.outcome.tracks, playlists: result.outcome.playlists, missing: result.outcome.skippedAudio })}</p><code data-i18n-ignore>{result.outcome.destination}</code><p>{t("Structural checks passed. Test playback on your DJ player before performing.")}</p>{result.outcome.backupPath ? <><p>{t("Verified backup saved on this computer")}</p><code data-i18n-ignore>{result.outcome.backupPath}</code></> : null}<code data-i18n-ignore>{result.reportPath}</code></div> : null}
    {error ? <p role="alert" className={styles.warning}>{error}</p> : null}
  </div>;
}

export function UsbRecoveryDialog({ path, onClose }: { path: string; onClose: () => void }) {
  const t = useTranslation();
  const dialog = useRef<HTMLDialogElement>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    const element = dialog.current;
    const previous = document.activeElement;
    element?.showModal();
    return () => { element?.close(); if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, []);
  return <dialog ref={dialog} className={styles.dialog} aria-label={t("USB diagnosis and recovery")} onKeyDown={e => e.stopPropagation()} onCancel={e => { e.preventDefault(); if (!busy) onClose(); }}>
    <header><h2>{t("USB diagnosis and recovery")}</h2><button type="button" disabled={busy} onClick={onClose}>{t("Close")}</button></header>
    <UsbRecoveryPane initialPath={path} onBusyChange={setBusy} />
  </dialog>;
}
