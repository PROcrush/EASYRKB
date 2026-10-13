import { expect, it } from "vitest";
import { createMockBackend } from "./backend-mock";
import { TREE_ROOT } from "./types";

it("creates explicit snapshots and deletes them without touching the library", async () => {
  const backend = createMockBackend({ trackCount: 20, writable: true });
  expect(await backend.listBackups()).toEqual([]);
  const path = await backend.backUpLibrary();
  expect(path).toMatch(/\/rbexport-\d{8}-\d{4}\.zip$/);
  await backend.edits.createPlaylist("After backup", TREE_ROOT);
  const edited = await backend.playlistTree();
  const backups = await backend.listBackups();
  expect(backups).toHaveLength(1);
  expect(backups[0]).toMatchObject({ path, includesAnalysis: true });
  expect(backups[0]!.bytes).toBeGreaterThan(0);
  expect(backups[0]!.createdAt).toBeGreaterThan(0);
  expect(backend).not.toHaveProperty("restoreBackup");
  expect(backend).not.toHaveProperty("pickBackupZip");
  await backend.deleteBackup(path);
  expect(await backend.listBackups()).toEqual([]);
  expect(await backend.playlistTree()).toEqual(edited);
  await expect(backend.deleteBackup(path)).rejects.toThrow("Backup not found");
});
