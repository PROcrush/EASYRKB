/**
 * Checks the gzip sizes of dist/assets/*.js against the initial-chunk and
 * total JavaScript limits in perf-budgets.json; exits unsuccessfully on overflow.
 * Run: pnpm build, then pnpm budget (which invokes this script). Read-only.
 */
import { readFileSync, readdirSync, statSync } from "node:fs";
import { gzipSync } from "node:zlib";
import { join } from "node:path";
import { fileURLToPath, URL } from "node:url";

const root = (p: string) => fileURLToPath(new URL(`../${p}`, import.meta.url));
const budgets = JSON.parse(readFileSync(root("perf-budgets.json"), "utf8")) as {
  gates: { bundle: { initialChunkKbGz: number; totalKbGz: number } };
};
const limits = budgets.gates.bundle;

const dir = root("dist/assets");
let total = 0, initial = 0;
const rows: string[] = [];
for (const f of readdirSync(dir).filter((f) => f.endsWith(".js"))) {
  const gz = gzipSync(readFileSync(join(dir, f))).length / 1024;
  total += gz;
  if (/index-/.test(f)) initial = Math.max(initial, gz);
  rows.push(`  ${f}  ${gz.toFixed(1)} KB gz  (raw ${(statSync(join(dir, f)).size / 1024).toFixed(1)} KB)`);
}
console.log(rows.sort().join("\n"));
console.log(`initial ${initial.toFixed(1)} / ${limits.initialChunkKbGz} KB gz · total ${total.toFixed(1)} / ${limits.totalKbGz} KB gz`);

const fail: string[] = [];
if (initial > limits.initialChunkKbGz) fail.push(`initial chunk ${initial.toFixed(1)} KB gz > ${limits.initialChunkKbGz}`);
if (total > limits.totalKbGz) fail.push(`total ${total.toFixed(1)} KB gz > ${limits.totalKbGz}`);
if (fail.length) { console.error("BUNDLE BUDGET EXCEEDED:\n  " + fail.join("\n  ")); process.exit(1); }
