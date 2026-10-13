/** @vitest-environment jsdom */
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ExportTargetDialog } from "./ExportTargetDialog";
import { Localization } from "@/i18n";
import { DEFAULT_PREFERENCES, savePreferences } from "@/lib/preferences";
import chinese from "../../public/locales/zh-CN.json";

let host: HTMLDivElement;
let root: Root;
const choose = vi.fn();
beforeEach(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  HTMLDialogElement.prototype.showModal = vi.fn();
  HTMLDialogElement.prototype.close = vi.fn();
  choose.mockReset();
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => { act(() => root.unmount()); host.remove(); localStorage.clear(); vi.unstubAllGlobals(); });
function render(maximumCompatibility = false, conversionFormat: "wav" | "aiff" | "mp3" = "wav") {
  act(() => root.render(<ExportTargetDialog destinations="USB A" onChoose={choose}
    audioDefaults={{ maximumCompatibility, conversionFormat }} />));
}
function click(selector: string) {
  const el = host.querySelector<HTMLElement>(selector);
  expect(el, selector).not.toBeNull();
  act(() => el!.click());
}
function start() {
  const button = [...host.querySelectorAll<HTMLButtonElement>("button")].find(b => b.textContent === "Start export");
  act(() => button!.click());
}
it("explains legacy FLAC/ALAC limitations and preserves the original format by default", () => {
  render();
  expect(host.textContent).toContain("CDJ-2000NXS");
  expect(host.textContent).toContain("FLAC and ALAC");
  expect(host.querySelector<HTMLInputElement>('input[name="export-audio-conversion"][value="original"]')?.checked).toBe(true);
  start();
  expect(choose).toHaveBeenCalledWith({ libraryFormats: "both", compatibilityFormat: undefined });
});
it.each(["wav", "aiff", "mp3"] as const)("lets the user choose %s for this export", format => {
  render();
  click('input[value="deviceLibrary"]');
  click('input[name="export-audio-conversion"][value="convert"]');
  const select = host.querySelector<HTMLSelectElement>('select[aria-label="Convert to"]')!;
  act(() => { select.value = format; select.dispatchEvent(new Event("change", { bubbles: true })); });
  start();
  expect(choose).toHaveBeenCalledWith({ libraryFormats: "deviceLibrary", compatibilityFormat: format });
});
it("allows keeping originals for this export even when conversion is enabled in preferences", () => {
  render(true, "mp3");
  expect(host.querySelector<HTMLSelectElement>("select")?.value).toBe("mp3");
  click('input[name="export-audio-conversion"][value="original"]');
  start();
  expect(choose).toHaveBeenCalledWith({ libraryFormats: "both", compatibilityFormat: undefined });
});
it("cancel and Escape both return no export choice", () => {
  render();
  const cancel = [...host.querySelectorAll<HTMLButtonElement>("button")].find(b => b.textContent === "Cancel")!;
  act(() => cancel.click());
  expect(choose).toHaveBeenLastCalledWith(null);
  choose.mockReset();
  act(() => { host.querySelector("dialog")!.dispatchEvent(new Event("cancel", { bubbles: false, cancelable: true })); });
  expect(choose).toHaveBeenLastCalledWith(null);
});
it("shows the compatibility notice and both choices in Simplified Chinese", async () => {
  savePreferences({ ...DEFAULT_PREFERENCES, view: { ...DEFAULT_PREFERENCES.view, locale: "zh-CN" } });
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: () => Promise.resolve(chinese) }));
  await act(async () => {
    root.render(<Localization><ExportTargetDialog destinations="USB A" onChoose={choose} /></Localization>);
    await new Promise(resolve => setTimeout(resolve, 0));
  });
  expect(host.textContent).toContain("FLAC、ALAC 无法在 CDJ-2000NXS 等部分老款碟机上播放");
  expect(host.textContent).toContain("保持原格式，不转换");
  expect(host.textContent).toContain("为老款碟机转换兼容格式");
  expect(host.querySelector<HTMLSelectElement>("select")?.getAttribute("aria-label")).toBe("转换为");
});
