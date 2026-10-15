/** @vitest-environment jsdom */
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { NcmConverter } from "./NcmConverter";
import { Localization } from "@/i18n";
import { DEFAULT_PREFERENCES, savePreferences } from "@/lib/preferences";
import chinese from "../../../public/locales/zh-CN.json";

const backend = vi.hoisted(() => ({ ncmSelectFiles: vi.fn(), ncmSelectOutput: vi.fn(), ncmConvertFile: vi.fn(), ncmOpenOutput: vi.fn() }));
vi.mock("@/ipc/client", () => ({ getBackend: () => Promise.resolve(backend) }));
let host: HTMLDivElement;
let root: Root;
const close = vi.fn();
beforeEach(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  HTMLDialogElement.prototype.showModal = vi.fn();
  vi.resetAllMocks();
  backend.ncmSelectFiles.mockResolvedValue(["/music/one.ncm", "/music/broken.ncm", "/music/one.ncm", "/music/other.mp3"]);
  backend.ncmSelectOutput.mockResolvedValue("/converted");
  backend.ncmConvertFile.mockImplementation((source: string) => {
    if (source.includes("broken")) return Promise.reject(new Error("The NCM file is damaged or incomplete."));
    return Promise.resolve({ outputPath: source.replace("/music/", "/converted/").replace(/ncm$/i, "flac"), format: "flac", bytes: 1234 });
  });
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
});
afterEach(() => { act(() => root.unmount()); host.remove(); localStorage.clear(); vi.unstubAllGlobals(); });
function button(text: string) { return [...host.querySelectorAll<HTMLButtonElement>("button")].find(b => b.textContent === text)!; }
async function click(text: string) { await act(async () => { button(text).click(); await Promise.resolve(); }); }
function render() { act(() => root.render(<NcmConverter onClose={close} />)); }

it("requires files and an output folder, deduplicates selection, and preserves cancelled picks", async () => {
  render(); expect(button("Start conversion").disabled).toBe(true);
  await click("Add NCM files");
  expect(host.querySelectorAll("tbody tr")).toHaveLength(2);
  expect(button("Start conversion").disabled).toBe(true);
  await click("Choose output folder");
  expect(button("Start conversion").disabled).toBe(false);
  backend.ncmSelectFiles.mockResolvedValue(null); backend.ncmSelectOutput.mockResolvedValue(null);
  await click("Add NCM files"); await click("Choose output folder");
  expect(host.querySelectorAll("tbody tr")).toHaveLength(2);
  expect(host.querySelector("input")?.value).toBe("/converted");
});

it("continues past a failed file and only retries unfinished files", async () => {
  backend.ncmSelectFiles.mockResolvedValue(["/music/broken.ncm", "/music/one.ncm"]);
  render(); await click("Add NCM files"); await click("Choose output folder"); await click("Start conversion");
  expect(backend.ncmConvertFile.mock.calls).toEqual([["/music/broken.ncm", "/converted"], ["/music/one.ncm", "/converted"]]);
  expect(host.querySelector('[data-status="failed"]')?.textContent).toContain("damaged or incomplete");
  expect(host.querySelector('[data-status="success"]')?.textContent).toContain("one.flac");
  backend.ncmConvertFile.mockResolvedValue({ outputPath: "/converted/broken.mp3", format: "mp3", bytes: 123 });
  await click("Retry unfinished files");
  expect(backend.ncmConvertFile).toHaveBeenCalledTimes(3);
  expect(host.querySelectorAll('[data-status="success"]')).toHaveLength(2);
  expect(button("Start conversion").disabled).toBe(true);
  await click("Open output folder"); expect(backend.ncmOpenOutput).toHaveBeenCalledWith("/converted");
});

it("stops after the current file and prevents closing while a conversion is running", async () => {
  let finish: (value: { outputPath: string }) => void = () => {};
  backend.ncmConvertFile.mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  render(); await click("Add NCM files"); await click("Choose output folder");
  await click("Start conversion");
  expect(button("Close").disabled).toBe(true);
  act(() => { host.querySelector("dialog")!.dispatchEvent(new Event("cancel", { cancelable: true })); });
  expect(close).not.toHaveBeenCalled();
  await click("Stop after current file");
  await act(async () => { finish({ outputPath: "/converted/one.flac" }); await Promise.resolve(); });
  expect(backend.ncmConvertFile).toHaveBeenCalledTimes(1);
  expect(host.querySelectorAll('[data-status="pending"]')).toHaveLength(1);
  expect(button("Close").disabled).toBe(false);
  await click("Close"); expect(close).toHaveBeenCalledTimes(1);
});

it("shows the conversion workflow and backend errors in Simplified Chinese", async () => {
  savePreferences({ ...DEFAULT_PREFERENCES, view: { ...DEFAULT_PREFERENCES.view, locale: "zh-CN" } });
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: () => Promise.resolve(chinese) }));
  await act(async () => { root.render(<Localization><NcmConverter onClose={close} /></Localization>); await Promise.resolve(); });
  expect(host.querySelector("dialog")?.getAttribute("aria-label")).toBe("NCM 格式转换");
  await click("添加 NCM 文件"); await click("选择输出文件夹"); await click("开始转换");
  expect(host.textContent).toContain("NCM 文件已损坏或不完整。");
  expect(host.textContent).toContain("成功 1 个 · 失败 1 个 · 共 2 个");
});
