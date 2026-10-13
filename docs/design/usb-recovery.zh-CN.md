# EASYRKB：U 盘诊断与修复

日期：2026-10-12。状态：已实现于工作区源码，已接入中文设置和设备右键菜单；按用户最新要求打包为 `downloads/EASYRKB.app`，原生设置内的修复页面已验证可打开。旧的 RBXport 中文版应用包保留，未包含本次功能；真实 U 盘与播放器硬件尚未验收。

## 1. 已实现的功能

入口：**优先选项 → USB 导出 → 打开 U 盘诊断与修复…**，或 **设备 → 右键 U 盘 → U 盘诊断与修复…**。打开设备列表和右键修复入口时不挂载设备设置面板，避免自动补写默认设置。

流程：选择源盘 → 只读诊断 → 用户选择 PDB 或 OneLibrary 来源 → 预览曲目和歌单 → 选择输出 → 核对后执行 → 校验及本机报告。

| 输出 | 实际行为 |
| --- | --- |
| 修复当前 U 盘（默认） | 先在电脑上备份并校验两份数据库及其附属文件；用所选可读库重建另一格式的库，在本机验证后发布到原盘，再回读检查 |
| 恢复到新空 U 盘 | 使用可读曲目和原有分析文件重建两份数据库，验证后发布到空目标；缺失音频跳过并报告 |
| 导出 XML | 保留原音频绝对路径、中文名称、歌单顺序及重复成员、空文件夹/歌单、可读 Cue、循环和变量拍网；缺失音频保留记录供重新定位 |

当前修复主要针对一份库损坏或双库内容不一致：选 PDB 时重建 `exportLibrary.db`；选 OneLibrary 时重建 `export.pdb`。修改前显示具体文件。所选数据库、音频、ANLZ 文件和其他原盘文件不在修复写入集合内。替换 OneLibrary 时，先备份再移除旧的 `-wal`、`-shm`、`-journal`，防止旧事务重放。

备份位于应用数据目录 `usb-recovery/backups/EASYRKB-before-repair-*`，包含原文件、SHA-256、缺失文件记录、来源与变更清单。结果页显示实际路径。诊断快照与 JSON 报告位于应用缓存目录 `usb-recovery`。

## 2. 上游代码审查

以下是固定提交的静态源码审查结果。未运行这些项目的测试，也未用用户 U 盘验证其恢复效果。

| 项目与审查提交 | 实际实现 | 对 RBXport 的用途 |
| --- | --- | --- |
| [rekordbox-explorer](https://github.com/CarlosFranzetti/rekordbox-explorer/tree/df7bb459450b94d8edf8e69eb913b21aa055c7a2) · MIT | 分别尝试 PDB、OneLibrary、Engine DJ，输出可读取的曲目及播放列表为 XML | 恢复来源发现、独立报告、失败来源隔离、恢复预览 |
| [DJ USB Tkit](https://github.com/haivala/dj-usb-tkit/tree/75be0fcd2eb0e882d61d222f43fd18d86ac5c76f) · AGPL-3.0-or-later | 诊断双库、预览修复目录、本机暂存数据库、应用选定修复、回写及复查 | 故障分类、修复依赖顺序、逐项结果与本机暂存思路 |
| [rekordbox-repair](https://github.com/edkennard/rekordbox-repair/tree/35763ca0a5b4ee2c775cd4a40bd8ac41718cf991) · MIT | Scala 读取已有 XML，搜索音乐文件，重写 Location | 路径重定位和重复候选处理规则 |
| [rekordbox-pdb](https://github.com/fragmede/rekordbox-pdb/tree/ee3bac2f22ca11a5ce61eea35f8cb951c246eaef) · MIT | Python PDB 读写，维护页堆、行目录、存在位及写入代次 | 格式对照与独立解析参考；RBXport 已有 Rust 等价模块 |

### Explorer：值得借鉴的恢复方法与限制

- [`recovery/index.ts`](https://github.com/CarlosFranzetti/rekordbox-explorer/blob/df7bb459450b94d8edf8e69eb913b21aa055c7a2/src/lib/recovery/index.ts#L121) 对每个来源分别捕获错误；一个数据库损坏不会阻止读取另一个数据库。这适合我们的恢复入口。
- 其推荐来源按“播放列表数量，再按曲目数量”排序。数量多不等于更完整、更新或属于同一曲库。我们的报告应另列有效成员数量、音频存在数量、数据来源与身份冲突，由用户查看后选择。
- [`assess.ts`](https://github.com/CarlosFranzetti/rekordbox-explorer/blob/df7bb459450b94d8edf8e69eb913b21aa055c7a2/src/lib/recovery/assess.ts#L59) 用页数及零页估算文件状态。这些是启发式线索，不能当作真实恢复率，也不能仅凭页边界或零页确定断电、未写入等原因。
- `-wal` / `-journal` 仅提供提示，当前恢复代码没有重放它们。若存在这些文件，我们必须一起保留，并把数据库读取与恢复操作放在本机副本中。
- [`engine.ts`](https://github.com/CarlosFranzetti/rekordbox-explorer/blob/df7bb459450b94d8edf8e69eb913b21aa055c7a2/src/lib/recovery/engine.ts#L86) 通过 SQLite 页遍历尝试读取存活数据。不能直接把其浏览器页遍历器当作通用损坏数据库恢复器：还需循环检测、资源上限、逐行错误报告、页边界验证和不完整记录隔离。
- [`xml.ts`](https://github.com/CarlosFranzetti/rekordbox-explorer/blob/df7bb459450b94d8edf8e69eb913b21aa055c7a2/src/lib/recovery/xml.ts#L63) 的路径固定为 macOS `/Volumes/卷名`，且曲目 XML 没有输出 `POSITION_MARK` 和 `TEMPO`。我们的实现须使用实际挂载路径，并报告 Cue / 拍网保存情况。

### Tkit：真正有修复代码，但需重新设计执行保障

核心文件是 [`service/diagnostics.rs`](https://github.com/haivala/dj-usb-tkit/blob/75be0fcd2eb0e882d61d222f43fd18d86ac5c76f/backend/src/service/diagnostics.rs)、[`service/repair.rs`](https://github.com/haivala/dj-usb-tkit/blob/75be0fcd2eb0e882d61d222f43fd18d86ac5c76f/backend/src/service/repair.rs) 与 [`service/usb_staging.rs`](https://github.com/haivala/dj-usb-tkit/blob/75be0fcd2eb0e882d61d222f43fd18d86ac5c76f/backend/src/service/usb_staging.rs)。

它能处理双库播放列表差异、PDB 页链中断、部分页头或页尾异常、路径重定位、未索引音频及分析文件问题。`repair_pdb_truncated_table_chain` 和 `repair_pdb_torn_growth_pages` 有先后依赖；不能把修复项当作互相独立的按钮。

几个不能照搬的实现细节：

- 空的 `selected_fix_ids` 会默认选择大多数支持的修复。我们的执行请求必须包含明确且非空的选项；用户全部取消时应执行零项。
- 修复入口把备份函数返回的文字追加到日志后继续。我们需要结构化的备份结果，备份和哈希校验成功才允许提交。
- 修复末尾分别回写 PDB 和 eDB；回写失败被记录为警告，单项失败也可能继续后续步骤。我们需要把完整变更集作为一个可恢复操作，禁止将一部分回写成功显示为全部修复成功。
- `wal_checkpoint` 的返回值在这一入口被忽略。我们的数据库发布必须确认 WAL 已处理且主文件包含最终事务。
- 文件大小、标题、艺术家的组合称为 fingerprint，但并非音频内容哈希；其文件名回退也只能作为候选匹配。错误文件匹配会把 Cue、拍网和播放列表关联到另一首歌，因此低置信候选必须人工选择。
- 有些规则专门纠正 Tkit 自己旧版生成的格式。我们的修复规则必须有明确格式证据和适用范围，不能对所有 rekordbox 导出一概套用。

### 另外两个项目

`rekordbox-repair` 的 [`Analyser.scala`](https://github.com/edkennard/rekordbox-repair/blob/35763ca0a5b4ee2c775cd4a40bd8ac41718cf991/src/main/scala/com/vividlab/rekordbox/analyse/Analyser.scala#L58) 按文件名搜索，唯一候选才重定位，多个候选留给用户；[`RekordBoxRewriteRule.scala`](https://github.com/edkennard/rekordbox-repair/blob/35763ca0a5b4ee2c775cd4a40bd8ac41718cf991/src/main/scala/com/vividlab/rekordbox/repair/RekordBoxRewriteRule.scala) 保留原 XML 的其他内容。值得吸收这种保留和歧义处理思路；不需要引入 JVM。

`rekordbox-pdb` 的 [`edit.py`](https://github.com/fragmede/rekordbox-pdb/blob/ee3bac2f22ca11a5ce61eea35f8cb951c246eaef/src/rekordbox_pdb/edit.py#L84) 是内存编辑器；`save()` 直接写文件，没有应用层的备份、分批发布和回滚流程。可用作格式对照，不能充当我们的修复执行器；不需要给 App 增加 Python 运行时。

## 3. 当前代码接入

| 文件 | 责任 |
| --- | --- |
| `crates/rbl-export/src/recovery.rs` | 本机快照、独立诊断、可信恢复模型、备份及原盘重建、新盘恢复、XML |
| `crates/rbl-pdb/src/lib.rs` | 页链循环、缺页、截断、异常行目录和偏移诊断；保留可读记录 |
| `crates/rbl-core/src/durable.rs` | `Publication::new_clean`：持有排他锁，但不自动重放旧日志或清理旧暂存文件 |
| `src-tauri/src/usb_recovery.rs` | 有数量上限的会话及任务、进度、取消、报告和命令注册 |
| `src/ipc/types.ts` / `client.ts` | 类型化 IPC；前端只提交来源 ID 和输出选项 |
| `src/views/devices/UsbRecovery.tsx` | 三步中文面板、缺失项、明确来源选择、变更预览及执行结果 |
| `src/views/settings/UsbExportPane.tsx` | 复用原 USB 导出设置分区 |
| `src/views/tree/TreeView.tsx` / `src/app/App.tsx` | 设备右键菜单及独立修复对话框 |
| `src/ipc/usb-recovery-mock.ts` | 浏览器专用故障示例，不接触真实设备或文件 |

正常的 `device_library::read`、`verify` 会自动恢复发布日志；诊断不调用它们。源库及 SQLite 附属文件先复制到本机，再用只读连接打开。诊断不创建源盘锁文件，不执行 checkpoint，不写设备默认设置。

## 4. 写入和失败规则

- 单个来源失败不阻止读取另一个来源；数量不会自动决定覆盖哪一版歌单。
- 设备身份、文件状态、数据库/分析文件哈希在快照及执行前后核验。变化时要求重新诊断。
- 拒绝源盘内的快照或备份目录，包括经符号链接进入源盘的新目录；限制数据库 64 MiB、单个分析资源 32 MiB、快照 1 GiB 和 100,000 条曲目。
- 修复先完成本机备份、哈希校验、文件刷新和生成库校验，才创建原盘发布暂存目录。
- 存在未完成的发布日志时，拒绝原盘修复并保留它；新盘/XML 可继续使用可读来源。
- 修复数据库发布和回读期间持有排他锁；写入前再次检查源库及将替换的文件。
- 复用现有 journal 发布机制。提交前可取消；进入提交后先完成当前发布，不把中断或部分写入显示成成功。
- 提交失败保留日志和本机备份，报告失败及备份位置。当前没有修复面板里的“一键回滚”入口。
- 新盘须为空且与源盘分离；XML 不覆盖已有文件。

## 5. 已验证结果与边界

新增 Rust 修复/恢复回归用例 **11 项**通过，覆盖双向单库损坏修复、原文件保留、本机备份、正常库拒绝无效重建、备份失败、取消、过期快照、源/目标边界、新盘歌单顺序及空歌单、未知 ANLZ section、旧 Cue 和变量拍网 XML。PDB 异常诊断用例 **3 项**通过，相关格式及发布回归测试通过。

前端 **1,181 项单元测试**、**3 项 Chromium 界面测试**、TypeScript、lint、Tauri/Rust 原生编译检查通过。中文审计参考本机 rekordbox 7.2.19.0342，新增文案已纳入人工词库和变量一致性检查。

这些验证使用临时目录和浏览器示例，没有操作用户真实 U 盘或曲库，也没有验证 CDJ/XDJ 硬件播放。用户说明见 `docs/USB修复.zh-CN.md`；具体日志在 `work/usb-repair-*.log`。

当前版本不会凭空恢复已经丢失的音频，也未实现文件系统/物理介质修复、深层 SQLCipher 页恢复、未索引音乐扫描、自动路径重定位或跨来源合并。两个数据库均不可读时，需已有可读副本或后续更深恢复适配。原始 Cue/拍网文件保留；缺失的标记无法由重新分析还原。

原盘修复保留所选库中的我的标签、设置、历史等原文件内容；重建库仅包含当前支持的字段。新盘重建不恢复我的标签、设备设置或播放历史；XML 无法表示所有播放器专用分析字段。

## 6. 实现和许可

使用项目已有 Rust 模块独立实现，以上外部项目用于源码行为和格式研究；没有复制 AGPL Tkit 修复代码。保留工作区的 LICENSE、LICENSING.md 和必要版权声明。
