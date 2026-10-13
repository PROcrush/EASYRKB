/**
 * The track list against a library that is not ready yet.
 *
 * The backend loads on its own thread, so the first `open_view` can arrive
 * before there is anything to answer with and comes back "The library has not
 * finished loading yet." `?slow` holds the mock back the same way, until
 * `window.__libraryReady()` is called.
 *
 * The tree already survived this; the table did not, and a list that opens
 * empty and stays empty is what a user sees as a blank window.
 */
import { expect, test, type Page } from "@playwright/test";

/**
 * Lets the mock's library finish loading.
 *
 * The backend module is imported lazily, so the hook does not exist until it
 * has landed — waiting for it is waiting for the same moment the real app
 * waits for.
 */
async function releaseLibrary(page: Page): Promise<void> {
  await page.waitForFunction(
    () => typeof (window as unknown as { __libraryReady?: unknown }).__libraryReady === "function",
  );
  await page.evaluate(() => (window as unknown as { __libraryReady: () => void }).__libraryReady());
}

test("a view opened before the library is ready fills in once it is", async ({ page }) => {
  await page.goto("/?slow&tracks=4000");

  // Nothing to draw yet, which is correct — the library genuinely is not up.
  await expect(page.locator('[role="row"] [data-col="title"]')).toHaveCount(0);

  await releaseLibrary(page);

  // The rows must arrive on their own. Nobody clicks anything here: the point
  // is that the view recovers without the user having to go somewhere else and
  // come back.
  await expect
    .poll(async () => page.locator('[role="row"] [data-col="title"]').count(), { timeout: 10_000 })
    .toBeGreaterThan(5);
  await expect(page.locator('[role="row"] [data-col="title"]').first()).not.toBeEmpty();
});

test("scrolling a recovered view still fetches the rows it lands on", async ({ page }) => {
  // The original report: pick the playlists root, drag the scrollbar halfway,
  // see nothing. A view that opened empty stays empty at every scroll position,
  // because its row count is zero and no window is ever asked for.
  await page.goto("/?slow&tracks=4000");
  await releaseLibrary(page);
  await expect
    .poll(async () => page.locator('[role="row"] [data-col="title"]').count(), { timeout: 10_000 })
    .toBeGreaterThan(5);

  await page.evaluate(() => {
    const el = document.querySelector('[data-testid="track-scroll"]');
    if (el) el.scrollTop = (el.scrollHeight - el.clientHeight) / 2;
  });

  await expect
    .poll(
      async () => {
        const t = await page.locator('[role="row"] [data-col="title"]').allTextContents();
        return t.length > 0 && t.every((x) => x.trim() !== "") ? t.length : 0;
      },
      { timeout: 10_000 },
    )
    .toBeGreaterThan(5);
});
