#!/usr/bin/env node

/**
 * Measures library first-paint, deck-frame, and scroll costs in Chromium
 * and WebKit with Playwright, printing a JSON report to stdout.
 * Run: pnpm perf:measure (builds first). Uses PERF_URL (default localhost:1421),
 * PERF_RUNS (default 5), and PERF_FRAMES (default 240); starts a Vite preview
 * when the target is unreachable and closes browsers and its preview afterward.
 */
import { spawn } from "node:child_process";
import { chromium, webkit } from "@playwright/test";

const url = process.env.PERF_URL ?? "http://127.0.0.1:1421";
const runs = Number.parseInt(process.env.PERF_RUNS ?? "5", 10);
const frames = Number.parseInt(process.env.PERF_FRAMES ?? "240", 10);
let preview;

if (!Number.isSafeInteger(runs) || runs < 3 || !Number.isSafeInteger(frames) || frames < 60) {
  throw new Error("PERF_RUNS must be at least 3 and PERF_FRAMES must be at least 60");
}

async function reachable() {
  try {
    return (await fetch(url)).ok;
  } catch {
    return false;
  }
}

async function ensurePreview() {
  if (await reachable()) return;
  const parsed = new URL(url);
  const executable = process.platform === "win32" ? "pnpm.cmd" : "pnpm";
  preview = spawn(executable, ["exec", "vite", "preview", "--host", parsed.hostname, "--port", parsed.port, "--strictPort", "--outDir", "dist"], {
    stdio: "ignore",
  });
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (await reachable()) return;
    if (preview.exitCode !== null) throw new Error("the production preview exited before it was ready");
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`production preview did not start at ${url}`);
}

function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)] ?? 0;
}

function rounded(value) {
  return Math.round(value * 100) / 100;
}

async function frameMedian(page) {
  const samples = await page.evaluate(async (count) => {
    const deltas = [];
    let last = performance.now();
    await new Promise((resolve) => {
      const tick = () => {
        const now = performance.now();
        deltas.push(now - last);
        last = now;
        if (deltas.length < count) requestAnimationFrame(tick);
        else resolve();
      };
      requestAnimationFrame(tick);
    });
    return deltas;
  }, frames);
  return median(samples);
}

async function openApp(browser) {
  const context = await browser.newContext({ viewport: { width: 1800, height: 1130 }, deviceScaleFactor: 2 });
  await context.addInitScript(() => {
    localStorage.setItem("rbl.preferences", JSON.stringify({ advanced: { protectLibrary: false } }));
  });
  const page = await context.newPage();
  await page.goto(url);
  await page.getByTestId("browser-title").waitFor();
  return { context, page };
}

async function startup(browser) {
  const samples = [];
  for (let run = 0; run < runs; run += 1) {
    const { context, page } = await openApp(browser);
    await page.waitForFunction(() => performance.getEntriesByName("startup:first-rows-painted").length === 1);
    samples.push(await page.evaluate(() => performance.getEntriesByName("startup:first-rows-painted")[0]?.startTime ?? 0));
    await context.close();
  }
  return samples;
}

async function deckCost(browser) {
  const samples = [];
  for (let run = 0; run < runs; run += 1) {
    const { context, page } = await openApp(browser);
    await page.getByRole("button", { name: "Layout" }).click();
    await page.getByRole("menuitemradio", { name: "FULL BROWSER" }).click();
    const empty = await frameMedian(page);

    await page.getByRole("button", { name: "Layout" }).click();
    await page.getByRole("menuitemradio", { name: "2 PLAYER" }).click();
    await page.locator('[role="gridcell"][data-col="title"]').first().dblclick();
    const cell = page.locator('[role="gridcell"][data-col="title"]').nth(1);
    await cell.click({ button: "right" });
    const menu = page.getByRole("menu", { name: "Track" });
    await menu.getByRole("menuitem", { name: "Load", exact: true }).hover();
    await menu.getByRole("menuitem", { name: "Load track to player 2" }).click();
    for (const name of ["Deck A transport", "Deck B transport"]) {
      await page.getByRole("group", { name }).getByRole("button", { name: "Play" }).click();
    }
    const both = await frameMedian(page);
    samples.push(both - empty);
    await context.close();
  }
  return samples;
}

async function scrollCost(browser) {
  const samples = [];
  for (let run = 0; run < runs; run += 1) {
    const { context, page } = await openApp(browser);
    await page.getByRole("treeitem", { name: /All Tracks/ }).click();
    const still = await frameMedian(page);
    const scrolling = await page.evaluate(async (count) => {
      const element = document.querySelector('[data-testid="track-scroll"]');
      if (!(element instanceof HTMLElement)) throw new Error("track scroller is missing");
      const deltas = [];
      let last = performance.now();
      let y = 0;
      let direction = 1;
      await new Promise((resolve) => {
        const tick = () => {
          const now = performance.now();
          deltas.push(now - last);
          last = now;
          const maximum = element.scrollHeight - element.clientHeight;
          y += 900 * direction;
          if (y >= maximum) {
            y = maximum;
            direction = -1;
          } else if (y <= 0) {
            y = 0;
            direction = 1;
          }
          element.scrollTop = y;
          if (deltas.length < count) requestAnimationFrame(tick);
          else resolve();
        };
        requestAnimationFrame(tick);
      });
      return deltas;
    }, frames);
    samples.push(median(scrolling) - still);
    await context.close();
  }
  return samples;
}

try {
  await ensurePreview();
  const results = {};
  for (const [name, browserType] of [["chromium", chromium], ["webkit", webkit]]) {
    const browser = await browserType.launch();
    try {
      const firstRows = await startup(browser);
      const decks = await deckCost(browser);
      const scroll = await scrollCost(browser);
      results[name] = {
        firstRowsMs: { median: rounded(median(firstRows)), max: rounded(Math.max(...firstRows)), samples: firstRows.map(rounded) },
        deckFrameCostMs: { median: rounded(median(decks)), max: rounded(Math.max(...decks)), samples: decks.map(rounded) },
        scrollFrameCostMs: { median: rounded(median(scroll)), max: rounded(Math.max(...scroll)), samples: scroll.map(rounded) },
      };
    } finally {
      await browser.close();
    }
  }
  console.log(JSON.stringify({ url, runs, frames, results }, null, 2));
} finally {
  preview?.kill();
}
