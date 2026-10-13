import { beforeEach, expect, it, vi } from "vitest";
import chinese from "../../public/locales/zh-CN.json";
import { localizedDialog } from "./dialog";

const native = vi.hoisted(() => ({ open: vi.fn(), save: vi.fn(), ask: vi.fn(), message: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => native);
vi.mock("@/i18n", async () => ({
  ...await vi.importActual<typeof import("@/i18n")>("@/i18n"),
  loadCatalog: () => Promise.resolve(chinese),
}));
beforeEach(() => vi.clearAllMocks());

it("localizes native file-dialog titles and filters while preserving paths and extensions", async () => {
  const dialog = await localizedDialog();
  await dialog.open({ title: "Add music to the library", defaultPath: "/Users/dj/Music/Collapse.flac", filters: [{ name: "Audio", extensions: ["flac"] }] });
  expect(native.open).toHaveBeenCalledWith({ title: "向资料库添加音乐", defaultPath: "/Users/dj/Music/Collapse.flac", filters: [{ name: "音频", extensions: ["flac"] }] });
  await dialog.save({ title: "Export Chill as txt", defaultPath: "Chill.txt" });
  expect(native.save).toHaveBeenCalledWith({ title: "将 Chill 导出为 txt", defaultPath: "Chill.txt" });
});

it("translates native warning bodies and button labels", async () => {
  const dialog = await localizedDialog();
  await dialog.ask("Remove 3 tracks from the collection? This can’t be undone. The files stay where they are.", { kind: "warning", title: "Collection" });
  expect(native.ask).toHaveBeenCalledWith("从收藏夹移除 3 首曲目 吗？此操作无法撤消，原文件会保留。", {
    kind: "warning", title: "收藏夹", okLabel: "确定", cancelLabel: "取消",
  });
});
