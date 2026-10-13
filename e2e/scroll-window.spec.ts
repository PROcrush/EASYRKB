/**
 * A long list fills the rows it is scrolled to.
 *
 * **These do not reproduce the defect they were written for, and are kept as
 * plain coverage rather than as its guard.** The fetch effect used to key on
 * `items.length` — how *many* rows are on screen, which does not change while
 * scrolling — so moving the window asked for nothing and every row it landed
 * on stayed a placeholder: full height, working scrollbar, no content.
 *
 * Under Playwright it always recovered, in both engines, because something
 * re-renders the table after a scroll settles and that re-run caught the
 * window up. Measured: time-to-fill after an identical drag was 165 ms with
 * the bug and 175 ms without it, so no timing bound separates them either.
 * The packaged app gets no such rescue — a drag there ended on row 19,329
 * with the last request made for rows 17,065-17,105 and nothing asked again
 * for eight seconds, which is how the bug was found and how the fix was
 * checked. See TODO.md.
 *
 * What is left below is still worth running: a long list that never fills
 * would fail it.
 */
import { expect, test } from "@playwright/test";

const ROW_H = 25;

/** Waits until every row on screen has content. */
async function settled(page: import("@playwright/test").Page) {
  await expect
    .poll(
      async () => {
        const t = await page.locator('[role="row"] [data-col="title"]').allTextContents();
        return t.length > 0 && t.every((x) => x.trim() !== "") ? t.length : 0;
      },
      { timeout: 15_000 },
    )
    .toBeGreaterThan(5);
}

/**
 * Opens the Playlists root — every track in the library, which is the list the
 * report is about. Without this the default selection is the first playlist,
 * thirty rows that do not scroll, and nothing here proves anything.
 */
async function openWholeLibrary(page: import("@playwright/test").Page) {
  await page.locator('[role="treeitem"]', { hasText: /^Playlists$/ }).first().click();
  await expect
    .poll(async () => {
      const el = page.locator('[data-testid="track-scroll"]');
      return el.evaluate((e) => e.scrollHeight - e.clientHeight);
    })
    .toBeGreaterThan(100_000);
}

async function scrollToRow(page: import("@playwright/test").Page, row: number) {
  await page.evaluate(
    ([r, h]) => {
      const el = document.querySelector('[data-testid="track-scroll"]');
      if (el) el.scrollTop = r * h;
    },
    [row, ROW_H] as const,
  );
}

test("a settled list that is scrolled again fetches the new window", async ({ page }) => {
  await page.goto("/?tracks=40000");
  await settled(page);
  await openWholeLibrary(page);
  await settled(page);

  // Both positions are mid-list, so the same number of rows is on screen at
  // each: nothing about the window's *size* changes, only where it is. That is
  // the case the old dependency could not see.
  await scrollToRow(page, 400);
  await settled(page);
  await scrollToRow(page, 900);
  await settled(page);
});

test("a drag that outruns the fetches still fills where it stops", async ({ page }) => {
  await page.goto("/?tracks=40000&latency=120");
  await settled(page);
  await openWholeLibrary(page);
  await settled(page);

  await page.evaluate(async () => {
    const el = document.querySelector('[data-testid="track-scroll"]');
    if (!el) return;
    const to = (el.scrollHeight - el.clientHeight) / 2;
    for (let i = 1; i <= 60; i++) {
      el.scrollTop = (to * i) / 60;
      await new Promise((r) => requestAnimationFrame(() => r(null)));
    }
  });

  await settled(page);
});
