import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("rbl.preferences", JSON.stringify({ view: { locale: "zh-CN" }, advanced: { protectLibrary: false } }));
  });
});

test("Chinese library and preferences use rekordbox terms and preserve music titles", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("browser-title")).toContainText("曲目");
  await expect(page.locator('[data-kind="collection"]').first()).toContainText("播放列表");
  await expect(page.locator('[role="gridcell"][data-col="title"]').first()).toHaveText("Safe With Me (Extended Remix)");
  await expect(page.getByRole("button", { name: "资料库只读", exact: true })).toHaveAttribute("title", "rekordbox 正在运行，编辑已锁定。退出 rekordbox 后即可编辑。");
  await page.getByRole("banner").getByRole("button", { name: "设置", exact: true }).click();
  const preferences = page.getByRole("dialog", { name: "优先选项", exact: true });
  await expect(preferences).toBeVisible();
  await expect(preferences.getByRole("combobox", { name: "语言", exact: true })).toHaveValue("zh-CN");
  await expect(preferences.getByRole("option", { name: "Français", exact: true })).toHaveText("Français");
  await preferences.getByRole("tab", { name: "音频", exact: true }).click();
  await expect(preferences.getByText("释放时间", { exact: true })).toBeVisible();
  await expect(preferences.getByRole("slider", { name: "限幅器释放时间", exact: true })).toBeVisible();
  await page.screenshot({ path: "work/zh-translation-audit/audio-zh.png" });
});

test("Chinese duplicate scan has a complete localized sentence", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("browser-title")).toContainText("曲目");
  await page.getByRole("banner").getByRole("button", { name: "设置", exact: true }).click();
  const preferences = page.getByRole("dialog", { name: "优先选项", exact: true });
  await preferences.getByRole("tab", { name: "高级", exact: true }).click();
  await preferences.getByRole("tab", { name: "数据库", exact: true }).click();
  await preferences.getByRole("button", { name: "查找重复项", exact: true }).click();
  await expect(preferences).toContainText(/未找到曲目标题和表演者均相同的重复曲目|发现 .* 组重复曲目，共有 .* 个额外副本/);
  await expect(preferences).not.toContainText("警察");
  await expect(preferences).toContainText("仅显示前 20 组。");
  await expect(preferences.getByRole("button", { name: "删除", exact: true })).toBeVisible();
  await page.screenshot({ path: "work/zh-translation-audit/duplicates-zh.png" });
});

test("Chinese About and reports have no author promotions or remote submissions", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("banner").getByRole("button", { name: "设置", exact: true }).click();
  const preferences = page.getByRole("dialog", { name: "优先选项", exact: true });
  await preferences.getByRole("tab", { name: "关于", exact: true }).click();
  await expect(preferences.getByRole("heading", { name: "EASYRKB", level: 1, exact: true })).toBeVisible();
  await expect(preferences).toContainText("此版本的错误报告仅保存在本机，更新需手动安装。");
  await expect(preferences.locator('section[aria-label="关于"] button, section[aria-label="关于"] a')).toHaveCount(0);
  await preferences.getByRole("button", { name: "报告错误", exact: true }).click();
  const report = page.getByRole("dialog", { name: "报告错误", exact: true });
  await expect(report).toContainText("报告仅保存在本机，不会自动发送。");
  await expect(report.locator("iframe, input[type=email]")).toHaveCount(0);
  await expect(report.getByRole("button", { name: "保存并打开报告", exact: true })).toBeDisabled();
  await page.screenshot({ path: "work/zh-translation-audit/local-report-zh.png" });
});


test("USB export asks for equipment in Chinese before writing", async ({ page }) => {
  await page.goto("/?writable=1");
  await expect(page.getByTestId("browser-title")).toContainText("曲目");
  await page.getByRole("button", { name: "同步管理器", exact: true }).click();
  const manager = page.getByRole("dialog", { name: "同步管理器" });
  await manager.getByRole("checkbox", { name: "Melodic Vox", exact: true }).check();
  await manager.getByRole("checkbox", { name: "DJ STICK", exact: true }).check();
  await manager.getByRole("button", { name: "同步", exact: true }).click();
  const target = page.getByRole("dialog", { name: "选择播放设备" });
  await expect(target).toBeVisible();
  await expect(target.getByText("全部格式（推荐）", { exact: true })).toBeVisible();
  await expect(target.getByText("CDJ-3000 及更早设备", { exact: true })).toBeVisible();
  await target.screenshot({ path: "work/usb-export-target-zh.png" });
  await target.getByRole("button", { name: "取消", exact: true }).click();
  await expect(target).toBeHidden();
});
