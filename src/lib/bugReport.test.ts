import { describe, expect, it } from "vitest";
import { formatBugReportAttachment } from "./bugReport";

describe("local report attachment", () => {
  it("keeps the description, settings and diagnostic log in the saved report", () => {
    const preferences = { advanced: { protectLibrary: true } };
    const report = formatBugReportAttachment("System information\nready\n", "  Audio stopped  ", preferences);
    const details = report.match(/^Report details\n([\s\S]+?)\n\nSystem information/)?.[1];
    expect(JSON.parse(details ?? "{}")).toEqual({ description: "Audio stopped", preferences });
    expect(report).toContain("System information\nready\n");
  });
});
