import type { Backend, UsbRecoveryDiagnosis, UsbRecoveryProgress } from "./types";

/** Browser-only recovery fixture. It never reads or writes files or devices. */
export function createUsbRecoveryMock(): Pick<Backend, "usbRecoveryDiagnose" | "usbRecoveryPreview" | "usbRecoveryExecute" | "usbRecoveryCancel" | "usbRecoveryRelease" | "usbRecoveryOpenReport" | "usbRecoveryXmlPath" | "onUsbRecoveryProgress" | "usbRecoveryCacheInfo" | "usbRecoveryClearCache" | "onUsbRecoveryCacheChange"> {
  const reports = new Map<string, UsbRecoveryDiagnosis>();
  const listeners = new Set<(progress: UsbRecoveryProgress) => void>();
  const cancelled = new Set<string>();
  let next = 0;
  let jobs = 0;
  let bytes = 0;
  const cacheListeners = new Set<(cleared: boolean) => void>();
  const changed = (cleared = false) => { for (const listener of cacheListeners) listener(cleared); };
  const cacheInfo = () => ({ path: "/mock/EASYRKB/usb-recovery", bytes, busy: jobs > 0, sessions: reports.size });
  const run = async (jobId: string, stage: string) => {
    jobs++; changed();
    try {
    for (const listener of listeners) listener({ jobId, stage, done: 0, total: 1 });
    await new Promise(resolve => setTimeout(resolve, 20));
    if (cancelled.delete(jobId)) throw new Error("Recovery stopped.");
    for (const listener of listeners) listener({ jobId, stage, done: 1, total: 1 });
    } finally { jobs--; changed(); }
  };
  const source = (reportId: string, sourceId: string) => {
    const found = reports.get(reportId)?.report.sources.find(s => s.id === sourceId && s.status !== "unreadable");
    if (!found) throw new Error("Choose a readable recovery source.");
    return found;
  };
  return {
    usbRecoveryDiagnose: async (path, jobId) => {
      await run(jobId, "scanning");
      const base = { format: "OneLibrary", database: "PIONEER/rekordbox/exportLibrary.db", sha256: "a".repeat(64), status: "partial" as const, tracks: 3, playlists: 1, playlistEntries: 3, missingAudio: 1, analysisTracks: 2, cueTracks: 2, beatGridTracks: 2, requiredBytes: 1024 * 1024 * 100, issues: [] };
      const report: UsbRecoveryDiagnosis = { reportId: `mock-recovery-${++next}`, report: { sourcePath: path, pendingPublication: false, sourceDisagreement: false, sources: [
        { ...base, id: "standard-pdb", format: "Device Library", database: "PIONEER/rekordbox/export.pdb", status: "unreadable", tracks: 0, playlists: 0, issues: [{ code: "unreadable", detail: "Truncated database fixture" }] },
        { ...base, id: "standard-one", issues: [{ code: "analysisMissing", detail: "Contents/遗失曲目.mp3" }] },
      ] } };
      reports.set(report.reportId, report);
      bytes += base.requiredBytes; changed();
      return report;
    },
    usbRecoveryPreview: (reportId, sourceId, offset) => {
      source(reportId, sourceId);
      return Promise.resolve({ total: 3, offset, tracks: [
        { id: 1, title: "恢复示例 A", artist: "DJ", path: "/Contents/A.mp3", audioPresent: true, analysisFiles: 2, cues: 4, beats: 640 },
        { id: 2, title: "恢复示例 B", artist: "DJ", path: "/Contents/B.mp3", audioPresent: true, analysisFiles: 2, cues: 2, beats: 512 },
        { id: 3, title: "遗失曲目", artist: "DJ", path: "/Contents/遗失曲目.mp3", audioPresent: false, analysisFiles: 0, cues: 0, beats: 0 },
      ].slice(offset, offset + 100), playlists: [{ id: 10, parent: 0, name: "我的演出歌单", folder: false, entries: 3 }] });
    },
    usbRecoveryExecute: async (reportId, sourceId, jobId, kind, destination) => {
      const selected = source(reportId, sourceId);
      await run(jobId, kind === "repair" ? "backup" : kind === "xml" ? "xml" : "copying");
      return { outcome: { destination: kind === "repair" ? `${destination}/PIONEER/rekordbox/export.pdb` : destination, kind, tracks: kind === "usb" ? selected.tracks - selected.missingAudio : selected.tracks, playlists: selected.playlists, skippedAudio: selected.missingAudio, verified: true, backupPath: kind === "repair" ? "/mock/EASYRKB-before-repair" : null }, reportPath: `/mock/EASYRKB-${reportId}.json` };
    },
    usbRecoveryCancel: jobId => { cancelled.add(jobId); return Promise.resolve(); },
    usbRecoveryRelease: reportId => { reports.delete(reportId); return Promise.resolve(); },
    usbRecoveryCacheInfo: () => Promise.resolve(cacheInfo()),
    usbRecoveryClearCache: async () => {
      if (jobs) throw new Error("Wait for diagnosis or recovery to finish before clearing the cache.");
      reports.clear(); bytes = 0; changed(true);
      return await Promise.resolve(cacheInfo());
    },
    onUsbRecoveryCacheChange: listener => { cacheListeners.add(listener); return () => { cacheListeners.delete(listener); }; },
    usbRecoveryOpenReport: async () => {},
    usbRecoveryXmlPath: () => Promise.resolve("/Users/mock/Desktop/EASYRKB-recovery.xml"),
    onUsbRecoveryProgress: listener => { listeners.add(listener); return () => { listeners.delete(listener); }; },
  };
}
