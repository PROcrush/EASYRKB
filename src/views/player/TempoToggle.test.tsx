/** @vitest-environment jsdom */
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import { DEFAULT_PREFERENCES } from "@/lib/preferences";
import { PreferencesProvider } from "@/store/usePreferences";
import { TempoToggle } from "./TempoToggle";

declare global { var IS_REACT_ACT_ENVIRONMENT: boolean; }

let host: HTMLDivElement;
let root: Root;
const changed = vi.fn();
const update = vi.fn();

beforeEach(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  changed.mockClear();
  update.mockClear();
  act(() => root.render(
    <PreferencesProvider value={{ preferences: DEFAULT_PREFERENCES, update, reset: vi.fn() }}>
      <TempoToggle bpmX100={12800} baseBpmX100={12800} onBpmChange={changed} />
    </PreferencesProvider>,
  ));
});
afterEach(() => { act(() => root.unmount()); host.remove(); });

it("edits playback BPM on a double click without opening the slider", () => {
  const button = host.querySelector("button")!;
  act(() => { button.dispatchEvent(new MouseEvent("dblclick", { bubbles: true })); });
  const input = host.querySelector("input")!;
  expect(input.value).toBe("128.00");
  act(() => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, "130.50");
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
  act(() => { input.dispatchEvent(new FocusEvent("focusout", { bubbles: true })); });
  expect(changed).toHaveBeenCalledWith(130.5);
  expect(update).not.toHaveBeenCalled();
});

it("ignores an invalid tempo", () => {
  act(() => { host.querySelector("button")!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true })); });
  const input = host.querySelector("input")!;
  act(() => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, "300");
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
  act(() => { input.dispatchEvent(new FocusEvent("focusout", { bubbles: true })); });
  expect(changed).not.toHaveBeenCalled();
});
