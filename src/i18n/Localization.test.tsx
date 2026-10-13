/** @vitest-environment jsdom */
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Localization } from ".";
import { DEFAULT_PREFERENCES, savePreferences } from "@/lib/preferences";
import chinese from "../../public/locales/zh-CN.json";

let root: Root;
let host: HTMLDivElement;

const settle = async () => {
  await act(async () => { await new Promise((resolve) => setTimeout(resolve, 0)); });
};

beforeEach(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  localStorage.clear();
  savePreferences({ ...DEFAULT_PREFERENCES, view: { ...DEFAULT_PREFERENCES.view, locale: "zh-CN" } });
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: () => Promise.resolve(chinese) }));
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
  localStorage.clear();
  vi.unstubAllGlobals();
});

describe("Chinese localization", () => {
  it("translates controls and their labels but preserves user metadata and native language names", async () => {
    act(() => root.render(<Localization>
      <button title="Beat grid" aria-label="Beat grid" aria-valuetext="Both decks" aria-description="Edit the beat grid">Beat grid</button>
      <span data-i18n-ignore><span title="Collection" aria-label="Collection">Collection</span></span>
    </Localization>));
    await settle();
    expect(host.querySelector("button")?.textContent).toBe("节拍网格");
    expect(host.querySelector("button")?.getAttribute("aria-label")).toBe("节拍网格");
    expect(host.querySelector("button")?.getAttribute("aria-valuetext")).toBe("两个播放器");
    expect(host.querySelector("button")?.getAttribute("aria-description")).toBe("编辑节拍网格");
    const metadata = host.querySelector("[data-i18n-ignore] span");
    expect(metadata?.textContent).toBe("Collection");
    expect(metadata?.getAttribute("title")).toBe("Collection");
    expect(metadata?.getAttribute("aria-label")).toBe("Collection");
  });

  it("restores English when switching language without changing a matching track title", async () => {
    act(() => root.render(<Localization>
      <button>Collection</button><span data-i18n-ignore>Collection</span>
    </Localization>));
    await settle();
    expect(host.querySelector("button")?.textContent).toBe("收藏夹");
    act(() => {
      savePreferences(DEFAULT_PREFERENCES);
      window.dispatchEvent(new Event("rbl-preferences"));
    });
    await settle();
    expect(host.querySelector("button")?.textContent).toBe("Collection");
    expect(host.querySelector("[data-i18n-ignore]")?.textContent).toBe("Collection");
  });
});
