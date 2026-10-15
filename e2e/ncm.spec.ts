import { expect, test } from "@playwright/test";

test("opens the NCM converter from the Chinese main window", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("rbl.preferences", JSON.stringify({ view: { locale: "zh-CN" } })));
  await page.goto("/");
  await page.getByRole("button", { name: "NCM 格式转换", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "NCM 格式转换", exact: true });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText("添加 NCM 文件，开始转换", { exact: true })).toBeVisible();
  await expect(dialog.getByRole("button", { name: "开始转换", exact: true })).toBeDisabled();
  await expect(dialog.getByRole("textbox", { name: "输出文件夹", exact: true })).toHaveAttribute("placeholder", "请选择转换后音频的保存位置");
  await dialog.screenshot({ path: "work/ncm-converter-zh.png" });
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
});
