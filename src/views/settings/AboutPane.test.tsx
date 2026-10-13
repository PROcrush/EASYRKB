/** @vitest-environment jsdom */
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { __setBackend } from "@/ipc/client";
import type { Backend } from "@/ipc/types";
import { AboutPane } from "./AboutPane";

it("shows this build's version and licence without personal links or update requests", async () => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  const checkForUpdate = vi.fn();
  const openUrl = vi.fn();
  __setBackend({ appVersion: () => Promise.resolve("1.7.0"), checkForUpdate, openUrl } as unknown as Backend);
  const host = document.createElement("div");
  const root = createRoot(host);
  try {
    await act(async () => { root.render(<AboutPane />); await Promise.resolve(); });
    expect(host.querySelector("h1")?.textContent).toBe("EASYRKB");
    expect(host.querySelector('[data-testid="about-version"]')?.textContent).toBe("v1.7.0");
    expect(host.textContent).toContain("GNU General Public License");
    expect(host.textContent).toContain("This build uses local reports and manual updates.");
    expect(host.querySelectorAll("a, button, input, select")).toHaveLength(0);
    expect(checkForUpdate).not.toHaveBeenCalled();
    expect(openUrl).not.toHaveBeenCalled();
  } finally {
    act(() => root.unmount());
    __setBackend(null);
  }
});
