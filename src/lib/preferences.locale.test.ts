/** @vitest-environment jsdom */
import { afterEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_PREFERENCES, loadPreferences, preferredLocale, savePreferences } from "./preferences";

afterEach(() => {
  localStorage.clear();
  vi.restoreAllMocks();
});

describe("first-launch language", () => {
  it("recognizes OS language tags and the Chinese script and regional variants", () => {
    expect(preferredLocale(["zh-Hans-CN", "en-US"])).toBe("zh-CN");
    expect(preferredLocale(["zh_Hant_HK", "en-US"])).toBe("zh-TW");
    expect(preferredLocale(["zh-TW"])).toBe("zh-TW");
    expect(preferredLocale(["zh"])).toBe("zh-CN");
    expect(preferredLocale(["xx", "ja-JP"])).toBe("ja");
    expect(preferredLocale([])).toBe("en");
  });

  it("starts a fresh Chinese profile in Chinese while retaining library protection", () => {
    vi.spyOn(navigator, "languages", "get").mockReturnValue(["zh-CN"]);
    expect(loadPreferences().view.locale).toBe("zh-CN");
    expect(loadPreferences().advanced.protectLibrary).toBe(true);
  });

  it("keeps an explicitly saved language after the OS language changes", () => {
    vi.spyOn(navigator, "languages", "get").mockReturnValue(["zh-CN"]);
    savePreferences(DEFAULT_PREFERENCES);
    expect(loadPreferences().view.locale).toBe("en");
  });
});
