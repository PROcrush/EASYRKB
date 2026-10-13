/** @vitest-environment jsdom */
import { act, StrictMode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { DEFAULT_PREFERENCES, loadPreferences } from "@/lib/preferences";
import { usePreferencesStore, type PreferencesStore } from "./usePreferences";

let store: PreferencesStore;
let root: Root;
let host: HTMLDivElement;

function Harness() {
  store = usePreferencesStore(DEFAULT_PREFERENCES);
  return null;
}

beforeEach(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  localStorage.clear();
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  act(() => root.render(<StrictMode><Harness /></StrictMode>));
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
  localStorage.clear();
  vi.restoreAllMocks();
});

it("persists the final batched preferences and announces them once after rendering", () => {
  const observed: string[] = [];
  const listener = () => observed.push(loadPreferences().view.locale);
  const error = vi.spyOn(console, "error");
  window.addEventListener("rbl-preferences", listener);
  try {
    act(() => {
      store.update("view", { locale: "zh-CN" });
      store.update("view", { tooltips: true });
    });
    expect(loadPreferences().view).toMatchObject({ locale: "zh-CN", tooltips: true });
    expect(observed).toEqual(["zh-CN"]);
    expect(error).not.toHaveBeenCalled();
    act(() => store.reset("view"));
    expect(observed).toEqual(["zh-CN", "en"]);
  } finally {
    window.removeEventListener("rbl-preferences", listener);
  }
});
