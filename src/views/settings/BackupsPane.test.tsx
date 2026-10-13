// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { BackupsPane } from "./BackupsPane";

const held = vi.hoisted(() => ({
  backend: { backupSizes: vi.fn(), startBackup: vi.fn(), cancelBackup: vi.fn(), backupProgress: vi.fn(), backupDirectory: vi.fn(), listBackups: vi.fn(), backUpLibrary: vi.fn(), setBackupDirectory: vi.fn(), pickFolder: vi.fn(), deleteBackup: vi.fn(), confirm: vi.fn() },
}));
vi.mock("@/ipc/client", () => ({ getBackend: () => Promise.resolve(held.backend) }));
let host: HTMLDivElement;
let root: Root;
const entry = { path: "/backups/library-test.zip", name: "library-test.zip", bytes: 1048576, createdAt: 1700000000000, includesAnalysis: true };
const button = (name: string) => [...host.querySelectorAll("button")].find(b => b.textContent === name)!;
beforeEach(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  vi.resetAllMocks();
  held.backend.listBackups.mockResolvedValue([entry]);
  held.backend.backupDirectory.mockResolvedValue("/backups");
  held.backend.confirm.mockResolvedValue(false);
  held.backend.backupSizes.mockResolvedValue({ updatedAt: 1700000000000, trackCount: 1234, artwork: 32, vocals: 64, database: 1024, waveforms: 4096, cues: 256, beatGrids: 512, phrases: 128, other: 128 });
  held.backend.backupProgress.mockResolvedValue({ running: false, phase: "", copiedBytes: 0, totalBytes: 0, path: null, error: null });
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
});
afterEach(() => { act(() => root.unmount()); host.remove(); vi.useRealTimers(); });
it("cancelling delete leaves the backup untouched", async () => {
  await act(async () => { root.render(<BackupsPane />); await Promise.resolve(); });
  await act(async () => { button("Delete").click(); await Promise.resolve(); });
  expect(held.backend.confirm).toHaveBeenCalledTimes(1);
  expect(held.backend.deleteBackup).not.toHaveBeenCalled();
  expect(host.querySelectorAll("tbody tr")).toHaveLength(1);
});
it("offers no restore here and points to RBXport Restore instead", async () => {
  await act(async () => { root.render(<BackupsPane />); await Promise.resolve(); });
  expect(button("Restore")).toBeUndefined();
  expect(button("Restore from ZIP…")).toBeUndefined();
  const guidance = host.querySelector('[aria-label="Restore a backup"]');
  expect(guidance?.querySelector("h4")?.textContent).toBe("Restore a backup");
  expect(guidance?.textContent).toContain("open RBXport Restore");
  expect(guidance?.querySelector("button")).toBeNull();
  expect(button("Create backup").disabled).toBe(false);
  expect(button("Change folder…").disabled).toBe(false);
  expect(button("Delete").disabled).toBe(false);
});
it("allows creating a backup while the library is read-only", async () => {
  await act(async () => { root.render(<BackupsPane />); await Promise.resolve(); });
  expect(button("Create backup").disabled).toBe(false);
});
it("reconnects to a background job after reopening Preferences and can stop it", async () => {
  vi.useFakeTimers();
  const progress = { running: true, phase: "copying", copiedBytes: 50, totalBytes: 100, path: null, error: null, currentItem: "Database · master.db" };
  held.backend.startBackup.mockImplementation(() => {
    held.backend.backupProgress.mockResolvedValue(progress);
    return Promise.resolve();
  });
  await act(async () => { root.render(<BackupsPane />); await Promise.resolve(); });
  await act(async () => { button("Create backup").click(); await Promise.resolve(); });
  expect(button("Create backup")).toBeUndefined();
  expect(button("Delete").disabled).toBe(true);
  expect(host.querySelector('[role="status"]')?.textContent).toContain("50%");
  expect(host.querySelector('[aria-label="Current backup item"]')?.textContent).toBe("Database · master.db");
  held.backend.backupProgress.mockResolvedValue({ ...progress, currentItem: "Analysis files · USBANLZ/001/ANLZ0000.DAT" });
  await act(async () => { await vi.advanceTimersByTimeAsync(500); });
  expect(host.querySelector('[aria-label="Current backup item"]')?.textContent).toBe("Analysis files · USBANLZ/001/ANLZ0000.DAT");
  act(() => root.unmount());
  root = createRoot(host);
  await act(async () => { root.render(<BackupsPane />); await Promise.resolve(); });
  expect(host.querySelector('[role="status"]')?.textContent).toContain("50%");
  held.backend.cancelBackup.mockImplementation(() => {
    held.backend.backupProgress.mockResolvedValue({ ...progress, phase: "stopping" });
    return Promise.resolve();
  });
  await act(async () => { button("Stop backup").click(); await Promise.resolve(); });
  expect(button("Stopping…").disabled).toBe(true);
  expect(host.querySelector('[aria-label="Current backup item"]')).toBeNull();
  held.backend.backupProgress.mockResolvedValue({ ...progress, running: false, phase: "cancelled" });
  await act(async () => { await vi.advanceTimersByTimeAsync(500); });
  expect(host.querySelector('[role="status"]')?.textContent).toBe("Backup stopped.");
  expect(button("Create backup").disabled).toBe(false);
  expect(held.backend.startBackup).toHaveBeenCalledTimes(1);
  expect(held.backend.cancelBackup).toHaveBeenCalledTimes(1);
});
it("refreshes the saved backups when a background job completes", async () => {
  vi.useFakeTimers();
  held.backend.listBackups.mockResolvedValue([]);
  await act(async () => { root.render(<BackupsPane />); await Promise.resolve(); });
  held.backend.listBackups.mockResolvedValue([entry]);
  held.backend.backupProgress.mockResolvedValue({ running: false, phase: "complete", copiedBytes: 100, totalBytes: 100, path: entry.path, error: null });
  await act(async () => { await vi.advanceTimersByTimeAsync(500); });
  expect(host.querySelectorAll("tbody tr")).toHaveLength(1);
  expect(host.querySelector('[role="status"]')?.textContent).toBe("Backup created.");
  expect(held.backend.backupSizes).toHaveBeenCalledTimes(1);
});
it("shows the empty graph, legend and spinner during the initial calculation", async () => {
  held.backend.backupSizes.mockReturnValue(new Promise(() => {}));
  await act(async () => { root.render(<BackupsPane />); await Promise.resolve(); });
  const bar = host.querySelector('[role="img"]');
  expect(bar?.children).toHaveLength(0);
  expect(bar?.parentElement?.getAttribute("aria-busy")).toBe("true");
  expect(bar?.parentElement?.querySelector('[role="status"]')?.textContent).toBe("Calculating Rekordbox data size");
  expect(bar?.parentElement?.querySelector("svg")).not.toBeNull();
  expect(host.querySelector('[aria-label="Backup size breakdown"]')?.children).toHaveLength(8);
  expect(host.textContent).toContain("Last updated: —");
  expect(button("Refresh").disabled).toBe(true);
  expect(held.backend.backupSizes).toHaveBeenCalledWith(false);
});
it("refreshes explicitly and keeps the last successful reading on failure", async () => {
  await act(async () => { root.render(<BackupsPane />); await Promise.resolve(); });
  const previousTime = host.querySelector("time")?.dateTime;
  held.backend.backupSizes.mockRejectedValueOnce(new Error("unavailable"));
  await act(async () => { button("Refresh").click(); await Promise.resolve(); });
  expect(held.backend.backupSizes).toHaveBeenLastCalledWith(true);
  expect(host.querySelector("time")?.dateTime).toBe(previousTime);
  expect(host.querySelector('[role="img"]')?.getAttribute("aria-label")).toContain("Waveform previews: 4.0 KB");
  expect(host.textContent).toContain("1,234 tracks");
  held.backend.backupSizes.mockResolvedValue({ updatedAt: 1800000000000, trackCount: 1, artwork: 0, vocals: 0, database: 2048, waveforms: 0, cues: 0, beatGrids: 0, phrases: 0, other: 0 });
  await act(async () => { button("Refresh").click(); await Promise.resolve(); });
  expect(host.querySelector("time")?.dateTime).toBe(new Date(1800000000000).toISOString());
  expect(host.querySelector('[role="img"]')?.getAttribute("aria-label")).toContain("Database: 2.0 KB");
  expect(host.querySelector('[role="alert"]')).toBeNull();
  expect(host.textContent).toContain("2.0 KB total · 1 track");
});
it("keeps backup actions usable if sizes cannot be measured, and lets the user retry", async () => {
  held.backend.backupSizes.mockRejectedValueOnce(new Error("unavailable"));
  await act(async () => { root.render(<BackupsPane />); await Promise.resolve(); });
  expect(button("Create backup").disabled).toBe(false);
  expect(host.textContent).toContain("Couldn’t calculate Rekordbox data size.");
  await act(async () => { button("Refresh").click(); await Promise.resolve(); });
  expect(host.querySelector('[aria-label="Backup size breakdown"]')?.children).toHaveLength(8);
  expect(host.querySelector('[role="img"]')?.getAttribute("aria-label")).toContain("Waveform previews: 4.0 KB");
});
it("shows an empty size bar without invalid segment widths when there is no data", async () => {
  held.backend.backupSizes.mockResolvedValue({ updatedAt: 1700000000000, trackCount: 0, artwork: 0, vocals: 0, database: 0, waveforms: 0, cues: 0, beatGrids: 0, phrases: 0, other: 0 });
  await act(async () => { root.render(<BackupsPane />); await Promise.resolve(); });
  const bar = host.querySelector('[role="img"]');
  expect(bar?.getAttribute("aria-label")).toBe("No data to back up");
  expect(bar?.children).toHaveLength(0);
});

it("automatically refreshes the data bar when the saved reading becomes a week old", async () => {
  vi.useFakeTimers();
  const now = Date.now();
  const week = 7 * 24 * 60 * 60 * 1000;
  const previous = await held.backend.backupSizes(false);
  held.backend.backupSizes.mockClear();
  held.backend.backupSizes.mockResolvedValue({ ...previous, updatedAt: now - week + 60_000 });
  await act(async () => { root.render(<BackupsPane />); await Promise.resolve(); });
  expect(held.backend.backupSizes).toHaveBeenCalledTimes(1);
  await act(async () => { await vi.advanceTimersByTimeAsync(59_999); });
  expect(held.backend.backupSizes).toHaveBeenCalledTimes(1);
  held.backend.backupSizes.mockResolvedValue({ ...previous, updatedAt: now + 60_000, database: 2048 });
  await act(async () => { await vi.advanceTimersByTimeAsync(1); });
  expect(held.backend.backupSizes).toHaveBeenCalledTimes(2);
  expect(held.backend.backupSizes).toHaveBeenLastCalledWith(false);
  expect(host.querySelector('[role="img"]')?.getAttribute("aria-label")).toContain("Database: 2.0 KB");
});

it("shows the message from a structured backend delete error", async () => {
  held.backend.confirm.mockResolvedValue(true);
  held.backend.deleteBackup.mockRejectedValue({ kind: "internal", message: "Backup: Permission denied" });
  await act(async () => { root.render(<BackupsPane />); await Promise.resolve(); });
  await act(async () => { button("Delete").click(); await Promise.resolve(); });
  expect(host.querySelector('[role="alert"]')?.textContent).toBe("Backup: Permission denied");
  expect(button("Delete").disabled).toBe(false);
});

it("changes the default folder and refreshes the list without moving or deleting backups", async () => {
  held.backend.pickFolder.mockResolvedValue("/Volumes/Archive");
  held.backend.setBackupDirectory.mockImplementation(() => {
    held.backend.listBackups.mockResolvedValue([]);
    return Promise.resolve("/Volumes/Archive");
  });
  await act(async () => { root.render(<BackupsPane />); await Promise.resolve(); });
  await act(async () => { button("Change folder…").click(); await Promise.resolve(); });
  expect(held.backend.setBackupDirectory).toHaveBeenCalledWith("/Volumes/Archive");
  expect(held.backend.deleteBackup).not.toHaveBeenCalled();
  expect(host.querySelectorAll("tbody tr")).toHaveLength(0);
  expect(host.textContent).toContain("Default backup folder updated.");
  expect(host.querySelector('[role="link"]')?.textContent).toBe("/Volumes/Archive");
});

it("cancelling the folder picker does not change the destination", async () => {
  held.backend.pickFolder.mockResolvedValue(null);
  await act(async () => { root.render(<BackupsPane />); await Promise.resolve(); });
  await act(async () => { button("Change folder…").click(); await Promise.resolve(); });
  expect(held.backend.setBackupDirectory).not.toHaveBeenCalled();
  expect(held.backend.confirm).not.toHaveBeenCalled();
  expect(host.querySelector('[role="link"]')?.textContent).toBe("/backups");
});

it("a destination failure keeps the previous folder and makes actions available again", async () => {
  held.backend.pickFolder.mockResolvedValue("/Volumes/Archive");
  held.backend.setBackupDirectory.mockRejectedValue(new Error("The backup folder is not writable."));
  await act(async () => { root.render(<BackupsPane />); await Promise.resolve(); });
  await act(async () => { button("Change folder…").click(); await Promise.resolve(); });
  expect(host.querySelector('[role="alert"]')?.textContent).toContain("not writable");
  expect(host.querySelectorAll("tbody tr")).toHaveLength(1);
  expect(host.querySelector('[role="link"]')?.textContent).toBe("/backups");
  expect(button("Change folder…").disabled).toBe(false);
});
