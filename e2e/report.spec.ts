import { expect, test } from "@playwright/test";

test("reports stay local and never load a remote verification frame", async ({ page }) => {
  const externalRequests: string[] = [];
  page.on("request", request => {
    if (!new URL(request.url()).hostname.match(/^(localhost|127\.0\.0\.1)$/)) externalRequests.push(request.url());
  });
  await page.goto("/");
  await page.getByRole("contentinfo").getByRole("button", { name: "Report bug", exact: true }).click();
  const report = page.getByRole("dialog", { name: "Report bug", exact: true });
  await expect(report.locator("iframe, input[type=email]")).toHaveCount(0);
  await expect(report.getByRole("checkbox")).toBeChecked();
  await expect(report).toContainText("Reports stay on this Mac. Nothing is sent automatically.");
  const save = report.getByRole("button", { name: "Save and open report" });
  await expect(save).toBeDisabled();
  await report.getByLabel("What happened?").fill("Playback stopped after loading a track.");
  await expect(save).toBeEnabled();
  await save.click();
  await expect(report.getByRole("alert")).toHaveText("Opening the text editor requires the desktop app.");
  expect(externalRequests).toEqual([]);
  await report.getByRole("checkbox").uncheck();
  await expect(save).toBeEnabled();
  await report.getByRole("button", { name: "Close", exact: true }).click();
  await expect(report).toHaveCount(0);
});

test("Preferences offers the local report at the bottom of its sidebar", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("banner").getByRole("button", { name: "Settings" }).click();
  const preferences = page.getByRole("dialog", { name: "Preferences", exact: true });
  await preferences.getByRole("navigation", { name: "Preference panes" }).getByRole("button", { name: "Report bug" }).click();
  const report = page.getByRole("dialog", { name: "Report bug", exact: true });
  await expect(report).toBeVisible();
  await expect(report).toContainText("Nothing is sent automatically.");
  await report.getByRole("button", { name: "Close report" }).click();
  await expect(report).toHaveCount(0);
  await expect(preferences).toBeVisible();
});
