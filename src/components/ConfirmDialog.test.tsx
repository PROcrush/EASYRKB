/** @vitest-environment jsdom */
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it } from "vitest";

import { confirmHandler } from "@/ipc/confirmHost";
import { ConfirmHost } from "./ConfirmDialog";

declare global {
  var IS_REACT_ACT_ENVIRONMENT: boolean;
}

let host: HTMLDivElement;
let root: Root;

beforeEach(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  act(() => root.render(<ConfirmHost />));
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

const buttons = () => [...host.querySelectorAll("button")];

it("asks in its own window, with No focused, and answers with the button pressed", async () => {
  let answer: Promise<boolean> | undefined;
  act(() => { answer = confirmHandler()?.("Remove 495 tracks?"); });
  expect(host.textContent).toContain("Remove 495 tracks?");
  expect(document.activeElement?.textContent).toBe("No");
  act(() => buttons().find((b) => b.textContent === "Yes")?.click());
  await expect(answer).resolves.toBe(true);
  expect(host.textContent).toBe("");
});

it("answers no on Escape and asks a second question after the first", async () => {
  let first: Promise<boolean> | undefined;
  let second: Promise<boolean> | undefined;
  act(() => {
    first = confirmHandler()?.("One?", { yes: "OK", no: "Cancel" });
    second = confirmHandler()?.("Two?");
  });
  expect(host.textContent).toContain("One?");
  expect(buttons().map((b) => b.textContent)).toEqual(["Cancel", "OK"]);
  act(() => { window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })); });
  await expect(first).resolves.toBe(false);
  expect(host.textContent).toContain("Two?");
  act(() => buttons().find((b) => b.textContent === "Yes")?.click());
  await expect(second).resolves.toBe(true);
});
