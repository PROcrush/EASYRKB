import { afterEach, beforeEach, expect, it, vi } from "vitest";

const backend = vi.hoisted(() => ({ invoke: vi.fn(), mock: vi.fn() }));
vi.mock("./invoke", () => ({ tauriInvoke: () => Promise.resolve(backend.invoke) }));
vi.mock("./backend-mock", () => ({ createMockBackend: backend.mock }));

beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  vi.stubEnv("DEV", false);
  vi.stubEnv("VITE_RBL_MOCK", "");
  vi.stubGlobal("window", {});
});
afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
});

it("production asks the native database and preserves its errors instead of showing samples", async () => {
  backend.invoke.mockRejectedValueOnce(new Error("Library is unavailable"));
  const { getBackend } = await import("./client");
  await expect((await getBackend()).librarySummary()).rejects.toThrow("Library is unavailable");
  expect(backend.invoke).toHaveBeenCalledWith("library_summary");
  expect(backend.mock).not.toHaveBeenCalled();
});

it("a desktop launch uses the native database even if demo mode was requested", async () => {
  vi.stubEnv("VITE_RBL_MOCK", "1");
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  backend.invoke.mockResolvedValueOnce({ tracks: 3680 });
  const { getBackend } = await import("./client");
  await expect((await getBackend()).librarySummary()).resolves.toEqual({ tracks: 3680 });
  expect(backend.mock).not.toHaveBeenCalled();
});

it("browser samples require development or an explicitly requested demo build", async () => {
  vi.stubEnv("VITE_RBL_MOCK", "1");
  const sample = { librarySummary: () => Promise.resolve({ tracks: 3 }) };
  backend.mock.mockReturnValueOnce(sample);
  const { getBackend } = await import("./client");
  expect(await getBackend()).toBe(sample);
  expect(backend.invoke).not.toHaveBeenCalled();
});
