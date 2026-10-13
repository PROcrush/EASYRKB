import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "@/i18n";
import type { ExportLibraryFormats } from "@/ipc/types";
import type { Preferences } from "@/lib/preferences";
import styles from "./ExportTargetDialog.module.css";

type AudioDefaults = Pick<Preferences["usbExport"], "maximumCompatibility" | "conversionFormat">;
export interface ExportTargetChoice {
  libraryFormats: ExportLibraryFormats;
  compatibilityFormat: "wav" | "aiff" | "mp3" | undefined;
}
const DEFAULT_AUDIO: AudioDefaults = { maximumCompatibility: false, conversionFormat: "wav" };
const LIBRARY_CHOICES = [
  ["both", "Both formats (recommended)", "Device Library + OneLibrary. Use when equipment is mixed or unknown."],
  ["oneLibrary", "CDJ-3000X / OneLibrary equipment", "OneLibrary: also for XDJ-AZ, OPUS-QUAD and OMNIS-DUO."],
  ["deviceLibrary", "CDJ-3000 and earlier equipment", "Device Library: CDJ-3000, CDJ-2000NXS2, XDJ-RX3 and older players."],
] as const;

export function ExportTargetDialog({ destinations, audioDefaults = DEFAULT_AUDIO, onChoose }: {
  destinations: string;
  audioDefaults?: AudioDefaults;
  onChoose: (choice: ExportTargetChoice | null) => void;
}) {
  const t = useTranslation();
  const dialog = useRef<HTMLDialogElement>(null);
  const [format, setFormat] = useState<ExportLibraryFormats>("both");
  const [convert, setConvert] = useState(audioDefaults.maximumCompatibility);
  const [conversionFormat, setConversionFormat] = useState(audioDefaults.conversionFormat);
  useEffect(() => {
    const element = dialog.current;
    element?.showModal();
    return () => element?.close();
  }, []);
  return <dialog ref={dialog} className={styles.dialog} aria-labelledby="export-target-title"
    onCancel={event => { event.preventDefault(); onChoose(null); }}
    onKeyDown={event => event.stopPropagation()}>
    <h2 id="export-target-title" className={styles.title}>{t("Choose playback equipment")}</h2>
    <p className={styles.summary}>{t("Export to {device}", { device: destinations })}</p>
    <fieldset className={styles.choices}>
      <legend>{t("Player library format")}</legend>
      {LIBRARY_CHOICES.map(([value, label, description]) => <label key={value} className={styles.choice}>
        <input type="radio" name="export-library-format" value={value} checked={format === value} onChange={() => setFormat(value)} />
        <span><strong>{t(label)}</strong><small>{t(description)}</small></span>
      </label>)}
    </fieldset>
    <p className={styles.summary}>{t("Existing library formats on the USB will also be updated to keep them consistent. Audio files are shared by both formats.")}</p>
    <section className={styles.audio} aria-labelledby="export-audio-title">
      <h3 id="export-audio-title">{t("Audio format compatibility")}</h3>
      <p className={styles.warning}>{t("FLAC and ALAC cannot play on older players such as the CDJ-2000NXS. High-sample-rate or floating-point WAV/AIFF and some AAC/M4A files may also be incompatible. Support depends on the model and encoding.")}</p>
      <fieldset className={styles.choices}>
        <legend>{t("Audio format for this export")}</legend>
        <label className={styles.choice}>
          <input type="radio" name="export-audio-conversion" value="original" checked={!convert} onChange={() => setConvert(false)} />
          <span><strong>{t("Keep original formats")}</strong><small>{t("Use when your players support these files. Incompatible files may not play on older equipment.")}</small></span>
        </label>
        <label className={styles.choice}>
          <input type="radio" name="export-audio-conversion" value="convert" checked={convert} onChange={() => setConvert(true)} />
          <span><strong>{t("Convert incompatible copies for older players")}</strong><small>{t("Only export copies that need compatibility conversion are converted. Compatible files are copied as-is; your originals stay unchanged.")}</small></span>
        </label>
      </fieldset>
      <div className={styles.conversionFormat}>
        <label htmlFor="export-conversion-format">{t("Convert to")}</label>
        <select id="export-conversion-format" aria-label={t("Convert to")} disabled={!convert} value={conversionFormat}
          onChange={event => setConversionFormat(event.target.value === "mp3" || event.target.value === "aiff" ? event.target.value : "wav")}>
          <option value="wav">{t("WAV — larger files")}</option>
          <option value="aiff">{t("AIFF — larger files")}</option>
          <option value="mp3">{t("MP3 — 320 kbps")}</option>
        </select>
      </div>
      <p className={styles.summary}>{t("WAV/AIFF: 16-bit / 44.1 kHz. MP3: smaller, lossy files.")}</p>
      <p className={styles.summary}>{t("This choice applies only to this export and does not change your saved preferences.")}</p>
    </section>
    <div className={styles.buttons}>
      <button type="button" autoFocus onClick={() => onChoose(null)}>{t("Cancel")}</button>
      <button type="button" className={styles.primary} onClick={() => onChoose({ libraryFormats: format, compatibilityFormat: convert ? conversionFormat : undefined })}>{t("Start export")}</button>
    </div>
  </dialog>;
}

/** Shared by track/playlist export and the separate Sync Manager window. */
export function useExportTarget() {
  const [request, setRequest] = useState<{ destinations: string; audioDefaults: AudioDefaults } | null>(null);
  const pending = useRef<((choice: ExportTargetChoice | null) => void) | null>(null);
  useEffect(() => () => { pending.current?.(null); pending.current = null; }, []);
  const askExportTarget = useCallback((devices: string, audioDefaults: AudioDefaults = DEFAULT_AUDIO) => new Promise<ExportTargetChoice | null>(resolve => {
    if (pending.current) { resolve(null); return; }
    pending.current = resolve;
    setRequest({ destinations: devices, audioDefaults });
  }), []);
  const choose = useCallback((choice: ExportTargetChoice | null) => {
    pending.current?.(choice);
    pending.current = null;
    setRequest(null);
  }, []);
  return { askExportTarget, exportTargetDialog: request === null ? null : <ExportTargetDialog {...request} onChoose={choose} /> };
}
