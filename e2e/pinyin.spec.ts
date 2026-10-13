import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("rbl.preferences", JSON.stringify({ view: { locale: "zh-CN" }, advanced: { protectLibrary: true } })));
});

test("Chinese pinyin searches, local corrections and regeneration work with library protection enabled", async ({ page }) => {
  await page.goto("/?pinyinFixture=1");
  await page.getByRole("treeitem", { name: /全部曲目/ }).click();
  const search = page.getByRole("searchbox", { name: "搜索当前曲目列表", exact: true });
  for (const query of ["七里香", "qilixiang", "qi li xiang", "ZJL QLX", "cqsl", "nverqing"]) {
    await search.fill(query);
    await expect(page.getByTestId("browser-title")).toContainText("1 曲目");
    await expect(page.getByText(query === "cqsl" ? "重庆森林" : query === "nverqing" ? "女儿情" : "七里香", { exact: true })).toBeVisible();
  }
  await search.fill("cqsl");
  await page.getByText("重庆森林", { exact: true }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "修正读音", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "修正读音", exact: true });
  await dialog.getByRole("textbox", { name: "曲名读音", exact: true }).fill("zhong qing sen lin");
  await dialog.getByRole("button", { name: "保存读音", exact: true }).click();
  await expect(dialog.getByText("读音已保存，搜索索引已更新。", { exact: true })).toBeVisible();
  await dialog.getByRole("button", { name: "关闭", exact: true }).click();
  await search.fill("zqsl");
  await expect(page.getByText("重庆森林", { exact: true })).toBeVisible();
  await page.getByRole("banner").getByRole("button", { name: "设置", exact: true }).click();
  const preferences = page.getByRole("dialog", { name: "优先选项", exact: true });
  await preferences.getByRole("tab", { name: "中文曲库", exact: true }).click();
  await preferences.getByRole("button", { name: "为全库生成拼音", exact: true }).click();
  await expect(preferences.getByRole("status")).toContainText("3 首含中文名称");
  await preferences.getByRole("button", { name: "关闭", exact: true }).click();
  await expect(page.getByText("重庆森林", { exact: true })).toBeVisible();
});
