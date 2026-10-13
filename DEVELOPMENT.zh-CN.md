# EASYRKB 开发与发布

基于 chrisle/rbxport v1.6.0 二次开发。当前版本 1.8.2。

正式版默认读取本机 rekordbox 数据库，生产前端不包含演示后端。数据库和用户音乐不随源码或安装包发布。

使用仓库固定的 Node、pnpm 和 Rust 工具链，按 README 编译；GitHub Actions 在 macOS arm64 和 Windows x64 原生环境生成安装包。构建工作流仅由手动请求触发，不自动发布。

版本在 package.json、Tauri 配置、Cargo.toml、Cargo.lock 中统一。中文源表为 scripts/locales/zh-CN.json，发布表为 public/locales/zh-CN.json。

1.8.2 包含曲库拼音、USB 诊断修复、导出复用与容量估算、连续拍网 Sync、上下轨同步缩放等改动。发布页提供本版本的检查范围、哈希和安装包。源码测试及数据库结构验证不代表所有 DJ 播放器兼容。
