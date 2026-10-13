/**
 * The INFO tab beside the deck, against the mock backend.
 *
 * `Show INFO Panel` in `german.lang`: the third tab of the cue list. It
 * prints the loaded track's rating, colour, comment and the file's four
 * facts from `track_details`, the record the information panel reads, so
 * the two agree — and an edit made in that panel shows here without a
 * reload.
 */
import { expect, test, type Page } from "@playwright/test";

const player = (page: Page) => page.getByRole("region", { name: "Preview player" });
const side = (page: Page) => page.getByRole("complementary", { name: "Cue list" });
const tab = (page: Page, name: string) => side(page).getByRole("tab", { name, exact: true });
const memoryRows = (page: Page) =>
  side(page).getByRole("button", { name: /^Delete memory cue \d\d:\d\d:\d\d\d$/ });
const fileLines = (page: Page) => page.getByTestId("deck-info-file").locator("span");

/** Loads the fourth row — analysed, so it carries the mock's cues. */
async function load(page: Page, query = "") {
  await page.goto(`/${query}`);
  await expect(page.getByTestId("browser-title")).toContainText("Tracks)");
  await page.locator('[role="gridcell"][data-col="title"]').nth(3).dblclick();
  await expect(player(page).getByRole("button", { name: "Play", exact: true })).toBeEnabled();
}

test("the tabs are MEMORY, HOT CUE and INFO, and the deck opens on MEMORY", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("browser-title")).toContainText("Tracks)");
  const tabs = side(page).getByRole("tablist", { name: "Cue list view" });
  await expect(tabs.getByRole("tab")).toHaveText(["MEMORY", "HOT CUE", "INFO"]);
  await expect(tab(page, "MEMORY")).toHaveAttribute("aria-selected", "true");
});

test("INFO shows the loaded track's record, and MEMORY is as it was on the way back", async ({ page }) => {
  await load(page);
  await expect(memoryRows(page)).toHaveCount(1);
  const before = await side(page).getByText(/^\d\d:\d\d:\d\d\d$/).allTextContents();

  await tab(page, "INFO").click();
  await expect(tab(page, "INFO")).toHaveAttribute("aria-selected", "true");
  const info = page.getByTestId("deck-info");
  await expect(info).toBeVisible();
  // The manual's four file lines, in its order, each in its own units. Which
  // track is fourth is the mock's business — it makes up an MP3 or a WAV — so
  // what is pinned here is the shape of the record, not one track's codec.
  await expect(fileLines(page)).toHaveCount(4);
  await expect(fileLines(page).nth(0)).toHaveText(/^(MP3|WAV) File$/);
  await expect(fileLines(page).nth(1)).toHaveText(/^\d+\.\d MB$/);
  await expect(fileLines(page).nth(2)).toHaveText("44100 Hz");
  await expect(fileLines(page).nth(3)).toHaveText(/^\d+ kbps$/);
  // The rating and comment come from the row itself: the Comments column
  // is on by default.
  const comment = await page.locator('[role="gridcell"][data-col="comment"]').nth(3).innerText();
  await expect(page.getByTestId("deck-info-comment")).toHaveText(comment);
  await expect(info.getByLabel(/^Rating \d of 5$/)).toBeVisible();
  // The list is not on screen while INFO is.
  await expect(memoryRows(page)).toHaveCount(0);

  await tab(page, "MEMORY").click();
  await expect(memoryRows(page)).toHaveCount(1);
  expect(await side(page).getByText(/^\d\d:\d\d:\d\d\d$/).allTextContents()).toEqual(before);
});

test("with nothing loaded INFO shows dashes", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("browser-title")).toContainText("Tracks)");
  await tab(page, "INFO").click();
  await expect(fileLines(page)).toHaveText(["—", "—", "—", "—"]);
  await expect(page.getByTestId("deck-info-comment")).toHaveText("—");
  await expect(page.getByTestId("deck-info-color")).toBeEmpty();
  await expect(page.getByTestId("deck-info").getByLabel("Rating 0 of 5")).toBeVisible();
});

test("the record is not fetched for a tab nobody opened", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("browser-title")).toContainText("Tracks)");
  // The mock counts its `trackDetails` calls; with the information panel
  // closed the deck's INFO tab is the only reader.
  const calls = () => page.evaluate(() => (window as unknown as { __detailsFetches?: number }).__detailsFetches ?? 0);

  await page.locator('[role="gridcell"][data-col="title"]').nth(3).dblclick();
  await expect(player(page).getByRole("button", { name: "Play", exact: true })).toBeEnabled();
  await page.locator('[role="gridcell"][data-col="title"]').nth(5).dblclick();
  await expect(player(page).getByRole("button", { name: "Play", exact: true })).toBeEnabled();
  expect(await calls()).toBe(0);

  await tab(page, "INFO").click();
  await expect(fileLines(page).nth(2)).toHaveText("44100 Hz");
  expect(await calls()).toBe(1);
});

test("a colour picked in the information panel shows on the deck", async ({ page }) => {
  await load(page, "?writable=1");
  await tab(page, "INFO").click();
  // Just that the record arrived; this test is about the colour.
  await expect(fileLines(page).nth(0)).toHaveText(/^(MP3|WAV) File$/);

  // The information panel opens on the same row the deck holds.
  await page.waitForFunction(() => "__menu" in window);
  await page.evaluate(() => (window as unknown as { __menu: (id: string) => void }).__menu("info"));
  const panel = page.getByRole("complementary", { name: "Information" });
  await expect(panel).toBeVisible();
  await panel.getByRole("tab", { name: "Info" }).click();
  await panel.getByRole("combobox", { name: "Color" }).selectOption({ label: "Aqua" });
  await expect(page.getByRole("contentinfo")).toContainText("Color saved");

  // The deck refetched on the library's announcement, without a reload or a
  // tab switch.
  const color = page.getByTestId("deck-info-color");
  await expect(color).toHaveText("Aqua");
  await expect(color.locator("[data-color=Aqua]")).toBeVisible();
});
