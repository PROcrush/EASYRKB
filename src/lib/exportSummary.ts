/**
 * What to say when an export finishes.
 *
 * An export to a stick that already holds one is a sync: most of it is usually
 * "left alone", and saying "exported 200 tracks" when 198 of them were never
 * touched tells someone nothing about what just happened to their stick.
 */

export interface ExportCounts {
  libraryFormats?: "both" | "deviceLibrary" | "oneLibrary";
  tracks: number;
  reused: number;
  removed: number;
  playlistsAdded?: number;
  playlistsRemoved?: number;
  skipped: string[];
  failed?: string[];
  verified: boolean;
}

function plural(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}

export function exportSummary(name: string, report: ExportCounts, t: (text: string) => string = text => text): string {
  const copied = Math.max(0, report.tracks - report.reused);
  const parts = [
    t(`${name}: Updated ${t(plural(copied, "track"))}`),
    t(`Skipped ${t(plural(report.reused, "track"))} (no change)`),
    t(`Added ${t(plural(report.playlistsAdded ?? 0, "playlist"))}`),
    t(`Removed ${t(plural(report.playlistsRemoved ?? 0, "playlist"))}`),
  ];

  if (report.skipped.length > 0) {
    parts.push(t(`${t(plural(report.skipped.length, "track"))} missing`));
  }
  if (report.failed && report.failed.length > 0) {
    parts.push(t(`${t(plural(report.failed.length, "track"))} failed`));
  }
  if (!report.verified) parts.push(t("but the result did not read back"));
  if (report.libraryFormats) parts.push(t(report.libraryFormats === "both" ? "Device Library + OneLibrary updated" : report.libraryFormats === "oneLibrary" ? "OneLibrary updated" : "Device Library updated"));
  return `${parts.join(". ")}.`;
}
