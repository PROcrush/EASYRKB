import { afterEach, describe, expect, it, vi } from "vitest";
import { IpcError, tauriInvoke } from "./invoke";

const invoke = vi.fn<(...args: unknown[]) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]): Promise<unknown> => invoke(...args) }));

afterEach(() => invoke.mockReset());

describe("tauriInvoke", () => {
  it("turns the backend's error object into an Error that says what went wrong", async () => {
    invoke.mockRejectedValueOnce({ kind: "internal", message: "Cannot read analysis /x/ANLZ0000.DAT.", detail: "No such file" });
    const call = (await tauriInvoke())("sync_devices");
    await expect(call).rejects.toBeInstanceOf(IpcError);
    await expect(call).rejects.toMatchObject({
      message: "Cannot read analysis /x/ANLZ0000.DAT. No such file",
      kind: "internal",
      detail: "No such file",
    });
  });

  it("passes results and ordinary errors through", async () => {
    invoke.mockResolvedValueOnce(7);
    expect(await (await tauriInvoke())("n", { a: 1 })).toBe(7);
    expect(invoke).toHaveBeenCalledWith("n", { a: 1 });
    const boom = new Error("boom");
    invoke.mockRejectedValueOnce(boom);
    await expect((await tauriInvoke())("n")).rejects.toBe(boom);
  });

  it("accepts a bare string rejection", async () => {
    invoke.mockRejectedValueOnce("plain");
    await expect((await tauriInvoke())("n")).rejects.toMatchObject({ message: "plain" });
  });
});
