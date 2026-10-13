import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("rbl.preferences", JSON.stringify({ view: { locale: "zh-CN" }, advanced: { protectLibrary: false } }));
  });
});

test("diagnosis cache capacity is remembered and clearing invalidates the recovery preview", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("browser-title")).toContainText("曲目");
  await page.getByRole("banner").getByRole("button", { name: "设置", exact: true }).click();
  const preferences = page.getByRole("dialog", { name: "优先选项", exact: true });
  await preferences.getByRole("tab", { name: "USB 导出", exact: true }).click();
  const capacity = preferences.getByRole("combobox", { name: "单次诊断缓存上限", exact: true });
  await expect(capacity).toHaveValue("8");
  await capacity.selectOption("16");
  await preferences.getByRole("button", { name: "打开 U 盘诊断与修复…", exact: true }).click();
  await expect(capacity).toHaveValue("16");
  await preferences.getByRole("textbox", { name: "源盘路径", exact: true }).fill("/Volumes/缓存测试");
  await preferences.getByRole("button", { name: "开始只读诊断", exact: true }).click();
  await preferences.getByRole("radio").nth(1).check();
  await expect(preferences.getByText("恢复示例 A", { exact: true })).toBeVisible();
  await page.evaluate(() => { (window as unknown as { __confirmAnswers: boolean[] }).__confirmAnswers = [false, true]; });
  const clear = preferences.getByRole("button", { name: "清除诊断缓存", exact: true });
  await clear.click();
  await expect(preferences.getByText("恢复示例 A", { exact: true })).toBeVisible();
  await clear.click();
  await expect(preferences.getByText("恢复示例 A", { exact: true })).toHaveCount(0);
  await expect(preferences.getByText("本机诊断缓存占用：0.00 MiB", { exact: true })).toBeVisible();
  await expect(preferences.getByText("诊断缓存已清除，修复前请重新诊断 U 盘。", { exact: true })).toBeVisible();
  await expect(capacity).toHaveValue("16");
});

test("Chinese USB recovery isolates a damaged library and requires reviewed preview before XML output", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("browser-title")).toContainText("曲目");
  await page.getByRole("banner").getByRole("button", { name: "设置", exact: true }).click();
  const preferences = page.getByRole("dialog", { name: "优先选项", exact: true });
  await preferences.getByRole("tab", { name: "USB 导出", exact: true }).click();
  await preferences.getByRole("button", { name: "打开 U 盘诊断与修复…", exact: true }).click();
  const source = preferences.getByRole("textbox", { name: "源盘路径", exact: true });
  await source.fill("/Volumes/损坏源盘");
  await preferences.getByRole("button", { name: "开始只读诊断", exact: true }).click();
  await expect(preferences.getByText("无法读取", { exact: true })).toBeVisible();
  const choices = preferences.getByRole("radio");
  await expect(choices.nth(0)).toBeDisabled();
  await choices.nth(1).check();
  await expect(preferences.getByText("恢复示例 A", { exact: true })).toBeVisible();
  await preferences.getByText("可恢复的播放列表与文件夹", { exact: true }).click();
  await expect(preferences.getByText("♫ 我的演出歌单 (3)", { exact: true })).toBeVisible();
  await expect(preferences.getByRole("button", { name: "备份并修复当前 U 盘", exact: true })).toBeDisabled();
  await preferences.getByRole("combobox", { name: "恢复输出", exact: true }).selectOption("xml");
  const start = preferences.getByRole("button", { name: "选择目标并开始恢复…", exact: true });
  await preferences.getByRole("checkbox", { name: "我已核对来源、缺失文件和恢复输出。", exact: true }).check();
  await start.click();
  await expect(preferences.getByText("已保存 3 首曲目、1 个播放列表。缺失音频：1 个。", { exact: true })).toBeVisible();
  await expect(preferences.getByText("/Users/mock/Desktop/EASYRKB-recovery.xml", { exact: true })).toBeVisible();
  await page.screenshot({ path: "work/usb-recovery-preview.png" });
  // Changing the input invalidates both preview and acknowledgement.
  await source.fill("/Volumes/另一源盘");
  await expect(start).toHaveCount(0);
});

test("device context recovery opens without mounting the device settings panel", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("browser-title")).toContainText("曲目");
  await page.getByRole("tab", { name: "设备", exact: true }).click();
  const device = page.locator('[data-kind="device"]').first();
  await expect(device).toBeVisible();
  await device.click({ button: "right" });
  await page.getByRole("menuitem", { name: "U 盘诊断与修复…", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "U 盘诊断与修复", exact: true });
  await expect(dialog).toBeVisible();
  await expect(device).not.toHaveAttribute("data-selected");
  await expect(dialog.getByRole("textbox", { name: "源盘路径", exact: true })).not.toHaveValue("");
  await dialog.getByRole("button", { name: "关闭", exact: true }).click();
  await expect(dialog).toHaveCount(0);
});

test("current USB repair shows its change set and verified computer backup", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("browser-title")).toContainText("曲目");
  await page.getByRole("banner").getByRole("button", { name: "设置", exact: true }).click();
  const preferences = page.getByRole("dialog", { name: "优先选项", exact: true });
  await preferences.getByRole("tab", { name: "USB 导出", exact: true }).click();
  await preferences.getByRole("button", { name: "打开 U 盘诊断与修复…", exact: true }).click();
  await preferences.getByRole("textbox", { name: "源盘路径", exact: true }).fill("/Volumes/演出 U 盘");
  await preferences.getByRole("button", { name: "开始只读诊断", exact: true }).click();
  await preferences.getByRole("radio").nth(1).check();
  const repair = preferences.getByRole("button", { name: "备份并修复当前 U 盘", exact: true });
  await expect(repair).toBeDisabled();
  await expect(preferences.getByText("将重建的数据库", { exact: false })).toContainText("PIONEER/rekordbox/export.pdb");
  await preferences.getByRole("checkbox", { name: "我已核对来源、缺失文件和恢复输出。", exact: true }).check();
  await repair.click();
  await expect(preferences.getByText("U 盘修复完成", { exact: true })).toBeVisible();
  await expect(preferences.getByText("已在电脑上保存校验通过的备份", { exact: true })).toBeVisible();
  await expect(preferences.getByText("/Volumes/演出 U 盘/PIONEER/rekordbox/export.pdb", { exact: true })).toBeVisible();
  await preferences.getByText("/mock/EASYRKB-before-repair", { exact: true }).scrollIntoViewIfNeeded();
  await page.screenshot({ path: "work/usb-repair-preview.png" });
});
