/**
 * What the decks cost per frame, against the same window with none drawn.
 *
 * Relative, not absolute. A headless browser's `requestAnimationFrame` is
 * jittery enough that nearly half the frames of an empty window come back over
 * 16.7 ms — an absolute budget here would fail for reasons that have nothing
 * to do with the app. What is worth gating is the *difference* the decks make,
 * which is stable, and which is the number the waveform work has to hold.
 */
import { expect, test, type Page } from "@playwright/test";
import budgets from "../perf-budgets.json" with { type: "json" };

/**
 * Run explicitly through `npm run perf:gate`.
 *
 * The gate is a *difference* of medians precisely so a headless browser's own
 * jitter cancels out, and that holds on a machine this process has to itself.
 * It does not hold under parallel browser workers: the difference picks up
 * contention rather than the decks. The dedicated command uses one worker and
 * runs Chromium and WebKit in sequence. The ordinary E2E suite skips this file.
 */
test.skip(
  process.env.RBXPORT_PERF !== "1",
  "run timing gates with npm run perf:gate",
);

/** Frames to time. At 60 Hz this is about four seconds a run. */
const FRAMES = 240;
const previewPort = (Number(process.env.E2E_PORT) || 1420) + 1;
const productionUrl = `http://127.0.0.1:${previewPort}`;

/**
 * The most the decks may add to a frame, in milliseconds.
 *
 * Measured at 0.1-0.3 ms on this machine with two decks playing, so this is
 * ten times the observed cost: it catches a waveform redraw that has started
 * costing whole milliseconds without failing on ordinary noise.
 */
function browserName(project: string): "chromium" | "webkit" {
  return project === "webkit" ? "webkit" : "chromium";
}

async function medianFrame(page: Page): Promise<number> {
  const frames = await page.evaluate(async (count: number) => {
    const deltas: number[] = [];
    let last = performance.now();
    await new Promise<void>((done) => {
      const tick = () => {
        const now = performance.now();
        deltas.push(now - last);
        last = now;
        if (deltas.length < count) requestAnimationFrame(tick);
        else done();
      };
      requestAnimationFrame(tick);
    });
    return deltas;
  }, FRAMES);
  const sorted = [...frames].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)] ?? 0;
}

async function choose(page: Page, label: string) {
  await page.getByRole("button", { name: "Layout" }).click();
  await page.getByRole("menuitemradio", { name: label }).click();
}

test("two decks playing cost no more per frame than none at all", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1800, height: 1130 });
  await page.goto(productionUrl);
  await expect(page.getByTestId("browser-title")).toContainText("Tracks)");

  // The baseline: the same window, the same list, no deck drawn.
  await choose(page, "FULL BROWSER");
  const empty = await medianFrame(page);

  await choose(page, "2 PLAYER");
  await page.locator('[role="gridcell"][data-col="title"]').first().dblclick();
  const cell = page.locator('[role="gridcell"][data-col="title"]').nth(1);
  await cell.click({ button: "right" });
  const menu = page.getByRole("menu", { name: "Track" });
  await menu.getByRole("menuitem", { name: "Load", exact: true }).hover();
  await menu.getByRole("menuitem", { name: "Load track to player 2" }).click();
  // The transport is drawn in the column the decks share, so it is found by
  // its own name rather than inside the deck's region.
  for (const name of ["Deck A transport", "Deck B transport"]) {
    await page.getByRole("group", { name }).getByRole("button", { name: "Play" }).click();
  }
  const both = await medianFrame(page);

  expect(both - empty).toBeLessThan(
    budgets.gates.browser.deckFrameCostMs[browserName(testInfo.project.name)],
  );
});

test("scrolling the track list costs no more per frame than sitting still", async ({ page }, testInfo) => {
  // The other place a list app drops frames. Rows are virtualised and keyed by
  // id, and this is the gate that says so: a hard scroll — a screenful and a
  // half every frame, all the way down — against the same window sitting
  // still.
  await page.setViewportSize({ width: 1800, height: 1130 });
  await page.goto(productionUrl);
  await expect(page.getByTestId("browser-title")).toContainText("Tracks)");
  // The whole collection, not the playlist the app opens on: thirty rows fit
  // on screen and a list that does not scroll measures nothing.
  await page.getByRole("treeitem", { name: /All Tracks/ }).click();
  await expect(page.getByTestId("browser-title")).toContainText("All Tracks");
  const still = await medianFrame(page);

  const scrolling = await page.evaluate(async (count: number) => {
    const el = document.querySelector('[data-testid="track-scroll"]');
    if (!(el instanceof HTMLElement)) throw new Error("track scroller is missing");
    const deltas: number[] = [];
    let last = performance.now();
    let y = 0;
    let direction = 1;
    let lowest = 0;
    let highest = 0;
    await new Promise<void>((done) => {
      const tick = () => {
        const now = performance.now();
        deltas.push(now - last);
        last = now;
        const maximum = el.scrollHeight - el.clientHeight;
        y += 900 * direction;
        if (y >= maximum) {
          y = maximum;
          direction = -1;
        } else if (y <= 0) {
          y = 0;
          direction = 1;
        }
        el.scrollTop = y;
        lowest = Math.min(lowest, y);
        highest = Math.max(highest, y);
        if (deltas.length < count) requestAnimationFrame(tick);
        else done();
      };
      requestAnimationFrame(tick);
    });
    const sorted = [...deltas].sort((a, b) => a - b);
    return { median: sorted[Math.floor(sorted.length / 2)] ?? 0, travel: highest - lowest };
  }, FRAMES);

  expect(scrolling.travel).toBeGreaterThan(10_000);
  expect(scrolling.median - still).toBeLessThan(
    budgets.gates.browser.scrollFrameCostMs[browserName(testInfo.project.name)],
  );
});
