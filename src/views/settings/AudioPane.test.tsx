/**
 * @vitest-environment jsdom
 *
 * The limiter's controls on the Audio pane: what the switch and the sliders
 * send up, when the sliders are usable, and what the readouts say. The
 * device list is a backend call the pane makes on its own; here the backend
 * has nothing to offer, which is the build-without-an-engine case.
 */
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { __setBackend } from "@/ipc/client";
import type { Backend, Limiter } from "@/ipc/types";
import { DEFAULT_LIMITER } from "@/store/useLimiter";
import { AudioPane } from "./AudioPane";

declare global {
  var IS_REACT_ACT_ENVIRONMENT: boolean;
}

let host: HTMLDivElement;
let root: Root;
let onLimiterChange: ReturnType<typeof vi.fn>;

function mount(limiter: Limiter, reduction = 0, peakLeft = 0, peakRight = 0) {
  act(() => {
    root.render(
      <AudioPane
        tab="configuration"
        limiter={limiter}
        onLimiterChange={onLimiterChange}
        reduction={reduction}
        peakLeft={peakLeft}
        peakRight={peakRight}
      />,
    );
  });
}

const settle = () =>
  act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });

const toggle = () => host.querySelector<HTMLInputElement>('input[role="switch"]');
const slider = (label: string) => host.querySelector<HTMLInputElement>(`input[aria-label="${label}"]`);

/**
 * Moves a range input the way a drag does: set the value, then fire input.
 * Through `valueAsNumber` rather than `value`, which React wraps on the
 * element to tell its own writes from the user's.
 */
function drag(input: HTMLInputElement, value: number) {
  act(() => {
    input.valueAsNumber = value;
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

beforeEach(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  onLimiterChange = vi.fn();
  __setBackend({
    audioDevices: () => Promise.resolve({ devices: [], default: null, chosen: null }),
  } as unknown as Backend);
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
  __setBackend(null);
});

describe("AudioPane's limiter controls", () => {
  it("shows the ceiling and release as the window writes them", async () => {
    mount(DEFAULT_LIMITER);
    await settle();
    expect(host.textContent).toContain("Ceiling 0.0 dB");
    expect(host.textContent).toContain("Release 250 ms");
    expect(toggle()?.checked).toBe(false);
    expect(slider("Limiter ceiling")?.disabled).toBe(true);
    expect(slider("Limiter release")?.disabled).toBe(true);
  });

  it("the switch sends the new enabled state alone", async () => {
    mount(DEFAULT_LIMITER);
    await settle();
    act(() => toggle()?.click());
    expect(onLimiterChange).toHaveBeenCalledWith({ enabled: true });
    expect(onLimiterChange).toHaveBeenCalledTimes(1);
  });

  it("the sliders send their number alone, in the engine's units", async () => {
    mount({ ...DEFAULT_LIMITER, enabled: true });
    await settle();
    const ceiling = slider("Limiter ceiling");
    const release = slider("Limiter release");
    if (!ceiling || !release) throw new Error("no sliders");
    expect([ceiling.min, ceiling.max, ceiling.step]).toEqual(["-12", "0", "0.1"]);
    expect([release.min, release.max, release.step]).toEqual(["10", "1000", "10"]);
    const input = slider("Limiter input gain");
    if (!input) throw new Error("no input gain slider");
    expect([input.min, input.max, input.step]).toEqual(["-24", "24", "0.1"]);
    drag(input, 6);
    expect(onLimiterChange).toHaveBeenLastCalledWith({ inputGainDb: 6 });
    drag(ceiling, -3);
    expect(onLimiterChange).toHaveBeenLastCalledWith({ ceilingDb: -3 });
    drag(release, 300);
    expect(onLimiterChange).toHaveBeenLastCalledWith({ releaseMs: 300 });
  });

  it("greys the sliders out while the limiter is off, but still shows the numbers", async () => {
    mount({ inputGainDb: 6, enabled: false, ceilingDb: -6, releaseMs: 500 });
    await settle();
    expect(toggle()?.checked).toBe(false);
    expect(slider("Limiter ceiling")?.disabled).toBe(true);
    expect(slider("Limiter release")?.disabled).toBe(true);
    expect(host.textContent).toContain("Ceiling -6.0 dB");
    expect(host.textContent).toContain("Release 500 ms");
    act(() => toggle()?.click());
    expect(onLimiterChange).toHaveBeenCalledWith({ enabled: true });
  });

  it("meters the output per channel and the reduction, each with its figure", async () => {
    mount({ ...DEFAULT_LIMITER, enabled: true }, 2.46, 0.5, 1);
    await settle();
    const meters = host.querySelectorAll<HTMLElement>("[role=meter]");
    expect([...meters].map((meter) => meter.getAttribute("aria-label"))).toEqual([
      "Output L",
      "Output R",
      "Gain reduction",
    ]);
    // Half is six decibels down: nine tenths of a sixty-decibel scale.
    expect(meters[0]?.getAttribute("aria-valuenow")).toBe("90");
    expect(meters[1]?.getAttribute("aria-valuenow")).toBe("100");
    expect(host.textContent).toContain("-6.0 dB");
    expect(host.textContent).toContain("0.0 dB");
    expect(host.textContent).toContain("−2.5 dB");
    // Reduction is on a twelve-decibel scale.
    expect(meters[2]?.getAttribute("aria-valuenow")).toBe("2.46");

    mount(DEFAULT_LIMITER, 0);
    expect(host.textContent).toContain("−∞ dB");
    expect(host.textContent).toContain("0.0 dB");
  });
});

it("reset restores ceiling and release without switching the limiter off", async () => {
  mount({ inputGainDb: 6, enabled: true, ceilingDb: -2, releaseMs: 500 });
  await settle();
  const reset = [...host.querySelectorAll("button")].find(button => button.textContent === "Reset settings");
  act(() => reset?.click());
  expect(onLimiterChange).toHaveBeenCalledWith({ inputGainDb: DEFAULT_LIMITER.inputGainDb, ceilingDb: DEFAULT_LIMITER.ceilingDb, releaseMs: DEFAULT_LIMITER.releaseMs });
});

it("shows bypassed reduction as zero while continuing to meter output", async () => {
  mount(DEFAULT_LIMITER, 4, 0.5, 0.5);
  await settle();
  expect(host.querySelector('[aria-label="Gain reduction"]')?.getAttribute("aria-valuenow")).toBe("0");
  expect(host.querySelector('[aria-label="Gain reduction"]')?.getAttribute("aria-valuetext")).toBe("Limiter off");
});
