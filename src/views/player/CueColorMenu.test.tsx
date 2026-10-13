/**
 * @vitest-environment jsdom
 *
 * Clicking away from the cue colour menu closes it and does nothing else: the
 * click must not reach what was under the pointer, such as a cue row that
 * would jump to its cue.
 */
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CueColorMenu } from "./CueColorMenu";

declare global {
  var IS_REACT_ACT_ENVIRONMENT: boolean;
}

let host: HTMLDivElement;
let root: Root;
const onClose = vi.fn();
const underneath = vi.fn();

const press = (el: Element) => {
  for (const type of ["mousedown", "mouseup", "click"]) {
    el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true }));
  }
};

beforeEach(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  act(() => root.render(
    <>
      <button id="row" onClick={underneath}>row</button>
      <CueColorMenu x={0} y={0} memory={false} onChoose={vi.fn()} onClose={onClose} />
    </>,
  ));
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
  onClose.mockReset();
  underneath.mockReset();
});

describe("CueColorMenu", () => {
  it("closes on a click outside without clicking what is underneath", () => {
    act(() => press(document.getElementById("row")!));
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(underneath).not.toHaveBeenCalled();
    // The menu closed, so the next click is an ordinary click again.
    act(() => root.render(<button id="row" onClick={underneath}>row</button>));
    act(() => press(document.getElementById("row")!));
    expect(underneath).toHaveBeenCalledTimes(1);
  });

  it("does not close on a click inside", () => {
    act(() => press(document.querySelector('[role="menu"]')!));
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe("CueColorMenu comments", () => {
  it("offers Add comments on a memory cue too, and starts editing when it is chosen", () => {
    const onComment = vi.fn();
    act(() => root.render(<CueColorMenu x={0} y={0} memory onChoose={vi.fn()} onComment={onComment} onClose={onClose} />));
    const item = [...document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')].find(b => b.textContent === "Add comments");
    expect(item?.disabled).toBe(false);
    act(() => item?.click());
    expect(onComment).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalled();
  });
});
