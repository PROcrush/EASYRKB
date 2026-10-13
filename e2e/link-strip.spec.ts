/**
 * The Pro DJ LINK strip along the bottom, against the mock's `?link=` network
 * (a browser has none of its own).
 *
 * The shape it must keep, from docs/screenshots 2026-09-13: nothing at all
 * until a device is heard; then a full-height strip with the LINK button alone
 * at its left; then, once on, a deck per player with the mixer between them,
 * each deck taking a dropped track.
 */
import { expect, test } from "@playwright/test";

test("draws no strip until a device is heard", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("grid")).toBeVisible();
  await expect(page.getByTestId("link-deck-strip")).toBeHidden();
});

test("is the LINK button alone in a full-height strip before LINK is on", async ({ page }) => {
  await page.goto("/?link=detected");
  await expect(page.getByRole("grid")).toBeVisible();

  const strip = page.getByTestId("link-deck-strip");
  await expect(strip).toBeVisible();
  await expect(page.getByTestId("link-button")).toHaveAttribute("aria-pressed", "false");
  // The height is carried whether or not LINK is on.
  expect((await strip.boundingBox())?.height).toBeGreaterThan(60);
  // No decks yet.
  await expect(page.getByLabel("Players on the link")).toBeHidden();
});

test("clicking LINK joins the network and shows the players either side of the mixer", async ({ page }) => {
  await page.goto("/?link=detected");
  await expect(page.getByRole("grid")).toBeVisible();
  await page.getByTestId("link-button").click();

  await expect(page.getByTestId("link-button")).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByLabel("Player 1")).toBeVisible();
  await expect(page.getByLabel("Player 2")).toBeVisible();
  await expect(page.getByLabel("Mixer 33")).toBeVisible();
  // rekordbox seats the mixer between the decks. Scoped to the strip: the
  // preview deck's own menu is labelled "Player menu" and would match too.
  const order = await page
    .getByTestId("link-deck-strip")
    .locator("[aria-label^='Player '], [aria-label^='Mixer ']")
    .evaluateAll((els) => els.map((el) => el.getAttribute("aria-label")));
  expect(order).toEqual(["Player 1", "Mixer 33", "Player 2"]);
});

test("the mixer's MASTER and LINK CUE lamps share one compact row", async ({ page }) => {
  await page.goto("/?link=on");
  await expect(page.getByRole("grid")).toBeVisible();

  const mixer = page.getByLabel("Mixer 33");
  const master = mixer.getByText("MASTER", { exact: true });
  const linkCue = mixer.getByText("LINK CUE", { exact: true });
  const [masterBox, cueBox, sameParent] = await Promise.all([
    master.boundingBox(),
    linkCue.boundingBox(),
    mixer.evaluate((element) => {
      const spans = [...element.querySelectorAll("span")];
      const masterLamp = spans.find((span) => span.textContent === "MASTER");
      const cueLamp = spans.find((span) => span.textContent === "LINK CUE");
      return masterLamp?.parentElement === cueLamp?.parentElement;
    }),
  ]);

  expect(masterBox).not.toBeNull();
  expect(cueBox).not.toBeNull();
  expect(sameParent).toBe(true);
  expect(masterBox?.y).toBeCloseTo(cueBox?.y ?? 0, 0);
  expect(masterBox?.x ?? 0).toBeLessThan(cueBox?.x ?? 0);
});

test("a deck takes a track dragged from the library; the mixer does not", async ({ page }) => {
  await page.goto("/?link=on");
  await expect(page.getByRole("grid")).toBeVisible();

  const row = page.getByRole("row").filter({ has: page.getByRole("gridcell") }).nth(1);
  const deck = page.getByLabel("Player 1");
  await expect(deck).not.toHaveAttribute("data-drop-target", "true");

  await row.hover();
  await page.mouse.down();
  await deck.hover();
  await expect(deck).toHaveAttribute("data-drop-target", "true");
  // A mixer cannot be loaded, so it never becomes a target.
  await expect(page.getByLabel("Mixer 33")).not.toHaveAttribute("data-drop-target", "true");
  await page.mouse.up();
});
