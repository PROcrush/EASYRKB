import process from "node:process";
import { expect, test } from "@playwright/test";

const preview = `http://localhost:${(Number(process.env.E2E_PORT) || 1420) + 1}/`;

test("the built Sync Manager loads its layout on a fresh window", async ({ page }) => {
  // A separate Tauri window starts here without visiting the main app first.
  // Dev-server CSS injection hides broken production lazy-import dependencies.
  await page.goto(`${preview}#sync`);
  const window = page.locator("[data-windowed]");
  await expect(window).toBeVisible();
  await expect(window).toHaveCSS("display", "flex");
  await expect(window).toHaveCSS("flex-direction", "column");
  const icons = window.locator("svg");
  await expect(icons.first()).toBeVisible();
  const widths = await icons.evaluateAll(elements => elements.map(el => el.getBoundingClientRect().width));
  expect(widths.every(width => width > 0 && width <= 32)).toBe(true);
});
