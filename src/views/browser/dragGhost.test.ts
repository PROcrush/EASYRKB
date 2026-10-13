/**
 * @vitest-environment jsdom
 */
import { describe, expect, it, vi } from "vitest";

import { setRowDragImage } from "./dragGhost";

function row(): HTMLElement {
  const host = document.createElement("div");
  const element = document.createElement("div");
  element.className = "row";
  element.setAttribute("draggable", "true");
  element.style.transform = "translate3d(0, 250px, 0)";
  element.innerHTML = '<div role="gridcell">Track</div>';
  host.appendChild(element);
  document.body.appendChild(host);
  return element;
}

describe("setRowDragImage", () => {
  it("hands the browser a faded, flat copy of the row beside it, then takes it away", () => {
    vi.useFakeTimers();
    const rafs: FrameRequestCallback[] = [];
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
      rafs.push(cb);
      return rafs.length;
    });
    const element = row();
    element.getBoundingClientRect = () =>
      ({ left: 10, top: 300, width: 900, height: 25 }) as DOMRect;
    element.parentElement!.getBoundingClientRect = () =>
      ({ left: 10, top: 100, width: 900, height: 5000 }) as DOMRect;
    const setDragImage = vi.fn();
    const transfer = { setDragImage } as unknown as DataTransfer;

    setRowDragImage(element, transfer, { x: 100, y: 310 });

    expect(setDragImage).toHaveBeenCalledTimes(1);
    const [ghost, x, y] = setDragImage.mock.calls[0] as [HTMLElement, number, number];
    // The hand stays where it took hold of the row.
    expect([x, y]).toEqual([90, 10]);
    // Beside the row, so it inherits the grid's columns; flat, faded, and not
    // itself draggable or read out.
    expect(ghost.parentElement).toBe(element.parentElement);
    expect(ghost.style.transform).toBe("none");
    // Over the row's own place, behind it: WebKit only pictures what it paints.
    expect(ghost.style.top).toBe("200px");
    expect(ghost.style.left).toBe("0px");
    expect(ghost.style.zIndex).toBe("-1");
    expect(ghost.style.opacity).toBe("0.45");
    expect(ghost.style.width).toBe("900px");
    expect(ghost.getAttribute("draggable")).toBeNull();
    expect(ghost.getAttribute("aria-hidden")).toBe("true");
    expect(ghost.textContent).toBe("Track");

    // Gone once the browser has its picture.
    expect(rafs).toHaveLength(1);
    rafs[0]?.(0);
    expect(ghost.isConnected).toBe(false);
    expect(element.isConnected).toBe(true);
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("does nothing for a row that is not in the document", () => {
    const loose = document.createElement("div");
    const setDragImage = vi.fn();
    setRowDragImage(loose, { setDragImage } as unknown as DataTransfer, { x: 0, y: 0 });
    expect(setDragImage).not.toHaveBeenCalled();
  });
});
