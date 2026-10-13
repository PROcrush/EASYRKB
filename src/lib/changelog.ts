/**
 * The release workflow's Markdown, as the Update Manager draws it.
 *
 * The generated notes use five things — a `## [x.y.z] — date` heading per
 * release, `### Added / Changed / Fixed` under it, `- ` bullets, paragraphs,
 * and `code` spans — so that is what this reads. No markdown library: one
 * would be forty kilobytes of parser for five constructs, and this file is
 * the only markdown the app ever shows.
 */

export type ChangelogBlock =
  | { kind: "release"; version: string; date: string | null }
  | { kind: "heading"; text: string }
  | { kind: "paragraph"; text: string }
  | { kind: "list"; items: string[] };

/** Where a markdown bullet's text starts, or -1 if the line is not one. */
function bulletStart(line: string): number {
  const match = /^\s*[-*]\s+/.exec(line);
  return match ? match[0].length : -1;
}

/** `## [0.6.0] — 2026-09-12 — not published` → version and date. */
function release(line: string): { version: string; date: string | null } | null {
  const match = /^##\s+\[([^\]]+)\](.*)$/.exec(line);
  if (!match) return null;
  const date = /\d{4}-\d{2}-\d{2}/.exec(match[2] ?? "");
  return { version: match[1] ?? "", date: date ? date[0] : null };
}

/** The section's markdown as blocks, in order. */
export function parseChangelog(markdown: string): ChangelogBlock[] {
  const blocks: ChangelogBlock[] = [];
  let paragraph: string[] = [];
  let list: string[] = [];
  // A bullet's continuation lines are indented; they belong to the item.
  let inList = false;

  const flush = () => {
    if (paragraph.length > 0) {
      blocks.push({ kind: "paragraph", text: paragraph.join(" ") });
      paragraph = [];
    }
    if (list.length > 0) {
      blocks.push({ kind: "list", items: list });
      list = [];
    }
    inList = false;
  };

  for (const raw of markdown.split("\n")) {
    const line = raw.replace(/\s+$/, "");
    if (line === "") {
      flush();
      continue;
    }
    const rel = release(line);
    if (rel) {
      flush();
      blocks.push({ kind: "release", ...rel });
      continue;
    }
    if (line.startsWith("### ")) {
      flush();
      blocks.push({ kind: "heading", text: line.slice(4).trim() });
      continue;
    }
    // Link definitions at the foot of the file are not prose.
    if (/^\[[^\]]+\]:\s/.test(line)) continue;
    const bullet = bulletStart(line);
    if (bullet >= 0) {
      if (paragraph.length > 0) flush();
      list.push(line.slice(bullet).trim());
      inList = true;
      continue;
    }
    if (inList && /^\s+/.test(raw) && list.length > 0) {
      list[list.length - 1] += " " + line.trim();
      continue;
    }
    if (list.length > 0) flush();
    paragraph.push(line.trim());
  }
  flush();
  return blocks;
}

/** The runs of an inline string: plain text and `code` spans. */
export type Span = { code: boolean; text: string };

export function spans(text: string): Span[] {
  const out: Span[] = [];
  const parts = text.split("`");
  parts.forEach((part, i) => {
    if (part === "") return;
    // An odd index is inside backticks — when they pair up. An unpaired
    // trailing backtick leaves its text plain rather than as code.
    const code = i % 2 === 1 && i < parts.length - 1;
    out.push({ code, text: part });
  });
  return out;
}

/** Bytes as the window shows them: `8.2 MB`. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "0 KB";
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
