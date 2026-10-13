# EASYRKB

<img src="public/branding/easyrkb-app-icon.png" alt="EASYRKB 图标" width="112" />

**面向排歌、曲库整理和 U 盘维护的精简版 rekordbox 工具。**

EASYRKB 基于开源 [rbxport](https://github.com/chrisle/rbxport) 二次开发，使用 Rust + Tauri + React，提供简体中文界面。重点放在本地曲库、双轨预听、U 盘导出和数据库诊断修复。

[下载安装包](https://github.com/PROcrush/EASYRKB/releases/latest) · [U 盘修复说明](docs/USB修复.zh-CN.md) · [中文拼音搜索](docs/中文曲库拼音.zh-CN.md) · [更新记录](release-notes.json)

## 默认打开你的数据库

安装后直接启动 EASYRKB，默认检测并读取本机 rekordbox 曲库；安装包不附带演示数据库或歌曲。找不到数据库时进入选择／创建曲库流程，读取失败时显示实际错误。已有曲库请先完成备份。

rekordbox 或其后台代理仍在运行时，EASYRKB 可能将曲库设为只读。需要编辑时，先正常退出 rekordbox 及其代理，再重新启动 EASYRKB。

## 功能

| 功能 | 用途 |
| --- | --- |
| 曲库和歌单整理 | 浏览、搜索、排序、创建歌单、编辑资料、定位缺失文件、导入／导出 XML。 |
| 中文歌曲搜索 | 保留原始中文曲名和表演者，支持全拼、首字母和组合搜索；离线生成索引，多音字可手动修正。 |
| 双轨预听与拍网编辑 | 波形、Cue、循环、变速、Sync 和拍网编辑；上下轨缩放始终一致。 |
| U 盘导出 | 导出所选曲目、歌单或同步范围；选择 Device Library、OneLibrary 或双库，复用已有音频和分析文件。 |
| 老设备音频兼容 | 导出前选择保持原格式，或生成 WAV、AIFF、320 kbps MP3 兼容副本；电脑原文件保留。实际支持取决于目标播放器。 |
| U 盘诊断和修复 | 独立只读检查两份库，预览可读曲目、歌单、Cue、拍网及缺失音频；备份后重建另一份数据库，或恢复到新空 U 盘、导出恢复 XML。 |
| 本地诊断与备份 | 诊断报告保存在本机，诊断缓存上限可调整和清理；修复前备份保留。 |
| 本地网络 LINK | 继承 rbxport 的 LINK 库服务功能，具体设备须单独验收。 |

U 盘修复针对可读来源下的数据库重建，不能恢复丢失音乐，也不能维修闪存或控制器故障。两份数据库均不可读时，需要可读备份。本项目不承诺所有 rekordbox 版本或所有 DJ 播放器兼容。

## 下载与安装

在 [Releases](https://github.com/PROcrush/EASYRKB/releases) 下载对应系统的安装包，并用 `SHA256SUMS.txt` 核对。

- **macOS Apple Silicon**：打开 `.dmg`，将 EASYRKB 拖入“应用程序”。当前使用本地临时签名，未经过 Apple 公证；系统可能提示无法验证开发者。
- **Windows x64**：运行中文 `setup.exe` 安装器。需要 Microsoft WebView2，安装器会处理缺少运行时的情况。当前未使用 Authenticode 签名，系统可能提示未知发布者。

公开版本使用手动下载安装更新。请按发布页的验证记录区分源码测试、安装包检查、实际 U 盘和播放器验收。

## 开发

需要 Node.js 24、pnpm 10.17.1、仓库指定的 Rust 工具链及 Tauri 平台依赖。桌面模式直接使用本机曲库：

```sh
pnpm install --frozen-lockfile
pnpm tauri dev --config src-tauri/tauri.zh.conf.json
```

浏览器演示只用于开发测试，需单独运行 `pnpm dev:mock` 或 `pnpm build:mock`，不作为桌面发行包。正式前端构建不会打入演示后端。

```sh
pnpm typecheck
pnpm test
pnpm build
```

macOS：`pnpm tauri build --bundles app --config src-tauri/tauri.zh.conf.json -- --locked`。
Windows：`pnpm tauri build --bundles nsis --config src-tauri/tauri.windows.zh.conf.json -- --locked`。
跨平台打包见 [EASYRKB 发布工作流](.github/workflows/easyrkb-release.yml)。原有上游发布流程依赖其服务器，不用于本仓库发布。

[架构说明](docs/development/architecture.md) · [开发说明](DEVELOPMENT.zh-CN.md) · [文档目录](docs/README.md)

## 来源、许可和商标

基于 Chris Le 及 rbxport 贡献者的开源代码。本项目及发行包保留 GPL 和第三方组件许可，完整对应源码随版本公开。见 [LICENSE](LICENSE) 和 [LICENSING.md](LICENSING.md)。

EASYRKB 是独立项目，与 AlphaTheta／Pioneer DJ 无隶属、授权或赞助关系。rekordbox、PRO DJ LINK 等名称及商标属于各自权利人。软件按 GPL 约定按现状提供，请保留曲库和 U 盘备份。
