import { expect, test } from "@playwright/test";

test("Preferences creates and deletes a library backup, and points restores to RBXport Restore", async ({ page }) => {
  await page.goto("/?writable=1");
  const settings = page.getByRole("banner").getByRole("button", { name: "Settings" });
  const preferences = page.getByRole("dialog", { name: "Preferences", exact: true });
  await settings.click();
  await expect(preferences.getByRole("tab", { name: "Time Machine", exact: true })).toHaveCount(0);
  await preferences.getByRole("tab", { name: "Backups", exact: true }).click();
  await expect(preferences.getByText("No backups yet.")).toBeVisible();
  const contents = preferences.getByRole("figure", { name: "Backup contents" });
  await expect(contents.getByRole("img")).toBeVisible();
  await expect(contents.getByText(/328.0 MB total · [\d,]+ tracks/)).toBeVisible();
  await expect(contents.getByRole("list", { name: "Backup size breakdown" }).getByRole("listitem")).toHaveCount(8);
  await preferences.getByRole("button", { name: "Create backup", exact: true }).click();
  await expect(preferences.getByText("Backup created.")).toBeVisible();
  const table = preferences.getByRole("table", { name: "Library backups" });
  await expect(table.locator("tbody tr")).toHaveCount(1);
  await expect(table.getByRole("columnheader")).toHaveText(["Date", "Time", "Size", "Actions"]);
  await expect(table.locator("time").first()).toHaveAttribute("datetime", /^\d{4}-/);
  await expect(table.getByRole("button", { name: "Restore", exact: true })).toHaveCount(0);
  await expect(preferences.getByRole("button", { name: "Restore from ZIP…", exact: true })).toHaveCount(0);
  await expect(preferences.getByRole("region", { name: "Restore a backup" })).toContainText("open RBXport Restore");
  await table.getByRole("button", { name: "Delete", exact: true }).click();
  await expect(preferences.getByText("Backup deleted.")).toBeVisible();
  await expect(preferences.getByText("No backups yet.")).toBeVisible();
});

test("backup progress survives closing Preferences and shows only a progress bar in the footer", async ({ page }) => {
  await page.clock.install();
  await page.goto("/?writable=1");
  const settings = page.getByRole("banner").getByRole("button", { name: "Settings" });
  const preferences = page.getByRole("dialog", { name: "Preferences", exact: true });
  const footer = page.getByRole("contentinfo");
  await settings.click();
  await preferences.getByRole("tab", { name: "Backups", exact: true }).click();
  await expect(preferences.getByText("No backups yet.")).toBeVisible();
  await page.clock.pauseAt(new Date(Date.now() + 1000));
  await preferences.getByRole("button", { name: "Create backup", exact: true }).click();
  await page.clock.runFor(600);
  await expect(preferences.getByRole("status")).toContainText("50%");
  const card = preferences.getByRole("region", { name: "Backup your Library" });
  await expect(card.getByRole("progressbar")).toBeVisible();
  await expect(card.getByLabel("Current backup item")).toHaveText("Analysis files · USBANLZ/001/ANLZ0000.DAT");
  await expect(card.getByRole("button", { name: "Stop backup", exact: true })).toBeVisible();
  await expect(footer.getByRole("progressbar", { name: "Backup progress" })).toHaveAttribute("value", "50");
  await expect(footer.getByText("Backup", { exact: true })).toBeVisible();
  await expect(footer.getByText("(50%)", { exact: true })).toBeVisible();
  await expect(footer.getByRole("button", { name: "Stop backup" })).toHaveCount(0);
  await expect(footer.getByRole("status")).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(preferences).not.toBeVisible();
  await expect(footer.getByRole("progressbar")).toBeVisible();
  await settings.click();
  await preferences.getByRole("tab", { name: "Backups", exact: true }).click();
  await preferences.getByRole("button", { name: "Stop backup", exact: true }).click();
  await page.clock.runFor(1000);
  await expect(footer.getByRole("status")).toHaveText("Backup stopped.");
  await expect(preferences.getByRole("status")).toHaveText("Backup stopped.");
  await expect(preferences.getByText("No backups yet.")).toBeVisible();
});


test("library editing is available before creating any backups", async ({ page }) => {
  await page.goto("/?writable=1");
  await page.getByRole("banner").getByRole("button", { name: "Settings" }).click();
  const preferences = page.getByRole("dialog", { name: "Preferences", exact: true });
  await preferences.getByRole("tab", { name: "Backups", exact: true }).click();
  await expect(preferences.getByText("No backups yet.")).toBeVisible();
  await page.keyboard.press("Escape");
  const title = page.locator('[role="gridcell"][data-col="title"]').nth(3);
  await title.click();
  await title.click();
  await title.locator("input").fill("Edited without a backup");
  await title.locator("input").press("Enter");
  await expect(title).toHaveText("Edited without a backup");
});

test("changing the default folder keeps old ZIPs in place and saves new backups there", async ({ page }) => {
  await page.goto("/?writable=1");
  const settings = page.getByRole("banner").getByRole("button", { name: "Settings" });
  const preferences = page.getByRole("dialog", { name: "Preferences", exact: true });
  await settings.click();
  await preferences.getByRole("tab", { name: "Backups", exact: true }).click();
  await preferences.getByRole("button", { name: "Create backup", exact: true }).click();
  const table = preferences.getByRole("table", { name: "Library backups" });
  await expect(table.locator("tbody tr")).toHaveCount(1);
  await preferences.getByRole("button", { name: "Change folder…", exact: true }).click();
  await expect(preferences.getByText("Default backup folder updated.")).toBeVisible();
  await expect(table).toHaveCount(0);
  await expect(preferences.getByRole("link", { name: "/Users/mock/Music/Moved", exact: true })).toBeVisible();
  await preferences.getByRole("button", { name: "Create backup", exact: true }).click();
  await expect(table.locator("tbody tr")).toHaveCount(1);
  await expect(table.getByRole("button", { name: "Move…", exact: true })).toHaveCount(0);
});
