# 简体中文界面

中文源表为 scripts/locales/zh-CN.json，发布表为 public/locales/zh-CN.json。共有功能术语参照 rekordbox 简体中文资源，自有功能按实际操作校订。

运行 node scripts/build-locales.mjs --locale=zh-CN 生成语言包。pnpm exec vitest run src/i18n/chinese.test.ts 可离线检查核心术语和动态提示。

安装了 rekordbox 的 macOS 开发机可运行 pnpm locales:audit:zh，对本机官方资源执行完整复核；原始官方资源不随仓库再分发。
