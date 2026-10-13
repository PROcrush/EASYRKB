import { describe, expect, it } from "vitest";
import chinese from "../../public/locales/zh-CN.json";
import { translate } from ".";
import { exportSummary } from "@/lib/exportSummary";
import { importSummary } from "@/lib/xmlImport";
import { nativeMenuLabels } from "@/lib/nativeMenu";

const t = (text: string) => translate(text, chinese);

describe("reviewed rekordbox Chinese", () => {
  it("uses the installed rekordbox terms for the same library controls", () => {
    expect(["Collection", "Explorer", "Preferences", "Artist", "Genre", "Track Title", "Master Tempo"].map(t))
      .toEqual(["收藏夹", "浏览器", "优先选项", "表演者", "风格", "曲目标题", "主拍速"]);
    expect(["8Bars", "Collapse", "Release", "Auto Relocate", "ALL DATA WILL BE ERASED"].map(t))
      .toEqual(["8小节", "折叠", "释放时间", "自动重新定位", "所有数据都将被抹掉"]);
    expect(nativeMenuLabels(t)).toMatchObject({ Preferences: "优先选项", "Sync Manager": "同步管理器", "Report bug": "报告错误" });
  });

  it("translates Rust positional errors without changing counts or source filenames", () => {
    expect(t("The USB did not verify after the sync: it lists 42 tracks where 40 were written."))
      .toBe("同步后 U 盘校验失败：实际列出 42 首曲目，应写入 40 首。");
    expect(t("Source unavailable for 'Chill'. Reconnect or relocate it before syncing; the USB has not been changed."))
      .toBe("“Chill”的源文件不可用。请重新连接或重新定位后再同步；U 盘尚未修改。");
  });

  it("keeps track titles intact when Chinese reorders named placeholders", () => {
    expect(t("Loop written: 8.25 s of Collapse."))
      .toBe("已写入循环：Collapse 中的 8.25 秒。");
    expect(t("Added 3 tracks to The Police Essentials; 2 skipped."))
      .toBe("已将 3 首曲目添加到 The Police Essentials；已跳过 2 首。");
  });

  it("translates composed duration, column labels and device summaries", () => {
    expect(t("Selected: 3 Tracks, 2 hours 7 minutes, 1.2 GB"))
      .toBe("已选择：3 首曲目，2 小时 7 分钟，1.2 GB");
    expect(t("Genre saved.")).toBe("已保存风格。");
    expect(t("Exporting 2 USBs")).toBe("正在导出 2 台 USB 设备");
    expect(t("Master tempo down")).toBe("降低主控拍速");
  });

  it("localizes the import and export report pieces rather than fragmented English words", () => {
    expect(exportSummary("USB A", { tracks: 5, reused: 2, removed: 0, playlistsAdded: 1, playlistsRemoved: 0, skipped: [], verified: true }, t))
      .toBe("USB A：已更新 3 首曲目. 已跳过 2 首曲目（未改变）. 已添加 1 个播放列表. 已移除 0 个播放列表.");
    expect(importSummary({ imported: 3, existing: 1, skipped: [], playlists: 2, cues: 4, tracks: [], sameNamed: [] }, t))
      .toContain("已导入 3 首曲目");
  });
});
