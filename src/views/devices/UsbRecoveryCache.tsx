import { useCallback, useEffect, useState } from "react";
import { getBackend } from "@/ipc/client";
import type { UsbRecoveryCacheInfo } from "@/ipc/types";
import { useTranslation } from "@/i18n";
import { DIAGNOSIS_CACHE_LIMITS } from "@/lib/preferences";
import { errorMessage } from "@/lib/errorMessage";
import { usePreferencesContext } from "@/store/usePreferences";
import styles from "./UsbRecovery.module.css";

export function UsbRecoveryCache({ busy = false }: { busy?: boolean }) {
  const t = useTranslation();
  const { preferences, update } = usePreferencesContext();
  const [info, setInfo] = useState<UsbRecoveryCacheInfo | null>(null);
  const [clearing, setClearing] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const refresh = useCallback(async () => setInfo(await (await getBackend()).usbRecoveryCacheInfo()), []);
  useEffect(() => {
    let live = true;
    let stop: (() => void) | undefined;
    const read = async () => {
      try { const next = await (await getBackend()).usbRecoveryCacheInfo(); if (live) setInfo(next); }
      catch (e) { if (live) setError(errorMessage(e)); }
    };
    void getBackend().then(backend => { if (live) { stop = backend.onUsbRecoveryCacheChange(() => { void read(); }); void read(); } }).catch(e => { if (live) setError(errorMessage(e)); });
    const onFocus = () => { void read(); };
    window.addEventListener("focus", onFocus);
    return () => { live = false; stop?.(); window.removeEventListener("focus", onFocus); };
  }, []);
  const clear = async () => {
    setError(""); setMessage("");
    try {
      const backend = await getBackend();
      if (!await backend.confirm(t("Clear local diagnosis snapshots and reports? USB files and repair backups are kept. Diagnose again before recovering."), { title: t("Clear diagnosis cache"), yes: t("Clear diagnosis cache"), no: t("Cancel") })) return;
      setClearing(true);
      setInfo(await backend.usbRecoveryClearCache());
      setMessage("Diagnosis cache cleared. Diagnose the drive again before recovering.");
    } catch (e) { setError(errorMessage(e)); }
    finally { setClearing(false); }
  };
  const disabled = busy || clearing || !info || info.busy;
  const size = info ? `${(info.bytes / (1024 ** (info.bytes >= 1024 ** 3 ? 3 : 2))).toFixed(2)} ${info.bytes >= 1024 ** 3 ? "GiB" : "MiB"}` : "…";
  return <div className={styles.pane}><fieldset disabled={busy || clearing}>
    <legend>{t("Diagnosis cache")}</legend>
    <label>{t("Diagnosis cache limit")}<select aria-label={t("Diagnosis cache limit")} disabled={disabled} value={preferences.usbExport.diagnosisCacheGiB} onChange={e => update("usbExport", { diagnosisCacheGiB: Number(e.target.value) })}>
      {DIAGNOSIS_CACHE_LIMITS.map(limit => <option key={limit} value={limit} data-i18n-ignore>{limit} GiB</option>)}
    </select></label>
    <p>{t("The limit applies to each diagnosis. Changes take effect when you diagnose again.")}</p>
    <p>{t("Local diagnosis cache: {size}", { size })}</p>
    {info ? <code data-i18n-ignore>{info.path}</code> : null}
    <div className={styles.actions}>
      <button type="button" disabled={disabled} onClick={() => { void clear(); }}>{t("Clear diagnosis cache")}</button>
      <button type="button" disabled={clearing} onClick={() => { void refresh().catch(e => setError(errorMessage(e))); }}>{t("Refresh cache usage")}</button>
    </div>
    <p>{t("Clearing removes local diagnosis snapshots and reports. USB files and repair backups are kept.")}</p>
    {info?.busy ? <p role="status">{t("Wait for diagnosis or recovery to finish before clearing the cache.")}</p> : null}
    {message ? <p role="status">{t(message)}</p> : null}
    {error ? <p role="alert" className={styles.warning}>{error}</p> : null}
  </fieldset></div>;
}
