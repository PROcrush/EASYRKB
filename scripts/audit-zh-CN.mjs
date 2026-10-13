/** Audit every shipped Simplified Chinese entry against the installed rekordbox. */
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const localePath = process.argv.find(arg => arg.startsWith("--source="))?.slice(9) ??
  "/Applications/rekordbox 7/rekordbox.app/Contents/Resources/locale/chinese_simplified.lang";
const resource = await readFile(localePath);
const official = new Map();
const decode = value => value.replaceAll("\\n", "\n").replaceAll('\\"', '"').replaceAll("\\\\", "\\");
let group = "";
resource.toString("utf8").split(/\r?\n/).forEach((line, index) => {
  if (line.startsWith("# group:")) group = line.slice(8).trim();
  const match = line.match(/^\s*"((?:\\.|[^"\\])*)"\s*=\s*"((?:\\.|[^"\\])*)"/);
  if (!match) return;
  const [key, translation] = match.slice(1).map(decode);
  if (!translation) return;
  const candidates = official.get(key) ?? [];
  candidates.push({ translation, group, line: index + 1 });
  official.set(key, candidates);
});
const readJson = async path => JSON.parse(await readFile(resolve(root, path), "utf8"));
const catalog = await readJson("public/locales/zh-CN.json");
const reviewed = await readJson("scripts/locales/zh-CN.json");
const policy = await readJson("scripts/locales/zh-CN-policy.json");
const ui = await readJson("src/i18n/ui.json");
const baseline = JSON.parse(execFileSync("git", ["show", "HEAD:public/locales/zh-CN.json"], { cwd: root, encoding: "utf8" }));
const preserved = new Set(policy.preserved);
const placeholders = value => (value.match(/\{[^}]*\}/g) ?? []).sort();
const errors = [];
for (const text of ui) if (catalog[text] === undefined) errors.push(`Missing UI string: ${text}`);
const records = Object.entries(catalog).map(([source, translation]) => {
  const candidates = official.get(source) ?? [];
  const exception = policy.officialExceptions[source];
  const matches = candidates.some(candidate => candidate.translation === translation);
  if (candidates.length && !matches && !exception) errors.push(`Undocumented official difference: ${source}`);
  if (JSON.stringify(placeholders(source)) !== JSON.stringify(placeholders(translation))) errors.push(`Placeholder mismatch: ${source}`);
  if (typeof translation !== "string" || translation.trim() === "") errors.push(`Empty translation: ${source}`);
  if (reviewed[source] !== translation) errors.push(`Generated entry differs from reviewed source: ${source}`);
  if (source === translation && !matches && !preserved.has(source)) errors.push(`Unreviewed English fallback: ${source}`);
  return {
    source, before: baseline[source] ?? null, translation,
    changed: baseline[source] !== translation,
    status: matches ? "rekordbox_exact" : exception ? "documented_correction" : preserved.has(source) ? "preserved_identifier_or_name" : "reviewed_rbxport_wording",
    official: candidates, note: exception ?? null,
    detectedUi: ui.includes(source),
  };
});
for (const key of Object.keys(reviewed)) if (catalog[key] === undefined) errors.push(`Reviewed entry not shipped: ${key}`);
const statusCounts = Object.fromEntries([...new Set(records.map(row => row.status))].map(status => [status, records.filter(row => row.status === status).length]));
const version = execFileSync("plutil", ["-extract", "CFBundleShortVersionString", "raw", resolve(dirname(localePath), "../../Info.plist")], { encoding: "utf8" }).trim();
const report = {
  checkedAt: new Date().toISOString(), timezone: "Asia/Shanghai",
  reference: { version, resource: localePath, sha256: createHash("sha256").update(resource).digest("hex") },
  counts: { total: records.length, detectedUi: ui.length, changed: records.filter(row => row.changed).length, new: records.filter(row => row.before === null).length, ...statusCounts },
  errors, records,
};
const out = resolve(root, "docs/localization");
await mkdir(out, { recursive: true });
await writeFile(resolve(out, "zh-CN-audit.json"), JSON.stringify(report, null, 2) + "\n");
const cell = value => '"' + String(value ?? "").replaceAll('"', '""') + '"';
const csv = [["英文原文", "修改前", "审定中文", "rekordbox 中文候选", "状态", "差异说明"],
  ...records.map(row => [row.source, row.before, row.translation, [...new Set(row.official.map(candidate => candidate.translation))].join(" / "), row.status, row.note])];
await writeFile(resolve(out, "zh-CN-audit.csv"), "\uFEFF" + csv.map(row => row.map(cell).join(",")).join("\r\n") + "\r\n");
console.log(JSON.stringify({ reference: report.reference, counts: report.counts, errors }, null, 2));
if (errors.length) process.exitCode = 1;
