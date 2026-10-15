import { afterEach, beforeEach, expect, it, vi } from "vitest";

const native = vi.hoisted(() => ({ invoke: vi.fn(), open: vi.fn() }));
vi.mock("./invoke", () => ({ tauriInvoke: () => Promise.resolve(native.invoke) }));
vi.mock("./dialog", () => ({ localizedDialog: () => Promise.resolve({ open: native.open }) }));
beforeEach(() => {
  vi.resetModules(); vi.clearAllMocks();
  vi.stubEnv("DEV", false); vi.stubEnv("VITE_RBL_MOCK", "");
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
});
afterEach(() => { vi.unstubAllEnvs(); vi.unstubAllGlobals(); });

it("uses native multi-file NCM selection and an explicit output-folder picker", async () => {
  const backend = await (await import("./client")).getBackend();
  native.open.mockResolvedValueOnce(["/music/歌曲.NCM"]).mockResolvedValueOnce("/output");
  await expect(backend.ncmSelectFiles()).resolves.toEqual(["/music/歌曲.NCM"]);
  expect(native.open).toHaveBeenNthCalledWith(1, { title: "Select NCM files", multiple: true, filters: [{ name: "NCM files", extensions: ["ncm"] }] });
  await expect(backend.ncmSelectOutput()).resolves.toBe("/output");
  expect(native.open).toHaveBeenNthCalledWith(2, { title: "Choose output folder", directory: true });
});

it("routes conversion and opening the output folder through registered native commands", async () => {
  const backend = await (await import("./client")).getBackend();
  const result = { outputPath: "/output/歌曲.flac", format: "flac", bytes: 42 };
  native.invoke.mockResolvedValueOnce(result).mockResolvedValueOnce(undefined);
  await expect(backend.ncmConvertFile("/music/歌曲.ncm", "/output")).resolves.toEqual(result);
  expect(native.invoke).toHaveBeenNthCalledWith(1, "ncm_convert_file", { source: "/music/歌曲.ncm", directory: "/output" });
  await backend.ncmOpenOutput("/output");
  expect(native.invoke).toHaveBeenNthCalledWith(2, "ncm_open_output", { directory: "/output" });
});
