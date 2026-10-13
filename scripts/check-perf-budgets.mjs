#!/usr/bin/env node

/**
 * Validates perf-budgets.json: its schema, positive limits, enforcement
 * settings, and recorded browser baselines against their gates.
 * Run: pnpm budget (also checks built bundle sizes). Read-only; this script
 * does not collect fresh performance measurements.
 */
import { readFile } from "node:fs/promises";

const path = new URL("../perf-budgets.json", import.meta.url);
const budgets = JSON.parse(await readFile(path, "utf8"));

function object(value, name) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${name} must be an object`);
  }
  return value;
}

function exactKeys(value, expected, name) {
  const actual = Object.keys(object(value, name)).sort();
  const wanted = [...expected].sort();
  if (actual.join("\n") !== wanted.join("\n")) {
    throw new Error(`${name} keys must be ${wanted.join(", ")}; found ${actual.join(", ")}`);
  }
}

function positive(value, name) {
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) {
    throw new Error(`${name} must be a positive number`);
  }
  return value;
}

exactKeys(budgets, ["$schemaVersion", "$comment", "baseline", "gates", "manualTargets", "enforcement"], "root");
if (budgets.$schemaVersion !== 2) throw new Error("$schemaVersion must be 2");
if (typeof budgets.$comment !== "string" || budgets.$comment.length === 0) throw new Error("$comment must describe the budget policy");

exactKeys(budgets.baseline, ["date", "sourceCommit", "environment", "bundle", "browser"], "baseline");
if (!/^\d{4}-\d{2}-\d{2}$/.test(budgets.baseline.date)) throw new Error("baseline.date must be YYYY-MM-DD");
if (!/^[0-9a-f]{7,40}$/.test(budgets.baseline.sourceCommit)) throw new Error("baseline.sourceCommit must be a Git commit");
if (typeof budgets.baseline.environment !== "string" || budgets.baseline.environment.length === 0) {
  throw new Error("baseline.environment must describe the measurement environment");
}

exactKeys(budgets.gates, ["bundle", "browser", "ipc"], "gates");
exactKeys(budgets.gates.bundle, ["initialChunkKbGz", "totalKbGz"], "gates.bundle");
exactKeys(budgets.gates.browser, ["firstRowsMs", "deckFrameCostMs", "scrollFrameCostMs"], "gates.browser");
exactKeys(budgets.gates.ipc, ["responseKb"], "gates.ipc");
for (const metric of ["firstRowsMs", "deckFrameCostMs", "scrollFrameCostMs"]) {
  exactKeys(budgets.gates.browser[metric], ["chromium", "webkit"], `gates.browser.${metric}`);
  for (const browser of ["chromium", "webkit"]) {
    positive(budgets.gates.browser[metric][browser], `gates.browser.${metric}.${browser}`);
  }
}

const measuredBundle = object(budgets.baseline?.bundle, "baseline.bundle");
for (const key of ["initialChunkKbGz", "totalKbGz"]) {
  const measured = positive(measuredBundle[key], `baseline.bundle.${key}`);
  const limit = positive(budgets.gates.bundle[key], `gates.bundle.${key}`);
  if (measured > limit) throw new Error(`baseline.bundle.${key} exceeds its gate`);
}

const measuredBrowsers = object(budgets.baseline?.browser, "baseline.browser");
exactKeys(measuredBrowsers, ["chromium", "webkit"], "baseline.browser");
for (const browser of ["chromium", "webkit"]) {
  const measured = object(measuredBrowsers[browser], `baseline.browser.${browser}`);
  exactKeys(
    measured,
    ["firstRowsMedianMs", "firstRowsMaxMs", "deckFrameCostMaxMs", "scrollFrameCostMaxMs"],
    `baseline.browser.${browser}`,
  );
  positive(measured.firstRowsMedianMs, `baseline.browser.${browser}.firstRowsMedianMs`);
  positive(measured.firstRowsMaxMs, `baseline.browser.${browser}.firstRowsMaxMs`);
  for (const key of ["deckFrameCostMaxMs", "scrollFrameCostMaxMs"]) {
    if (typeof measured[key] !== "number" || !Number.isFinite(measured[key]) || measured[key] < 0) {
      throw new Error(`baseline.browser.${browser}.${key} must be a non-negative number`);
    }
  }
  if (measured.firstRowsMaxMs > budgets.gates.browser.firstRowsMs[browser]) {
    throw new Error(`baseline.browser.${browser}.firstRowsMaxMs exceeds its gate`);
  }
  if (measured.deckFrameCostMaxMs > budgets.gates.browser.deckFrameCostMs[browser]) {
    throw new Error(`baseline.browser.${browser}.deckFrameCostMaxMs exceeds its gate`);
  }
  if (measured.scrollFrameCostMaxMs > budgets.gates.browser.scrollFrameCostMs[browser]) {
    throw new Error(`baseline.browser.${browser}.scrollFrameCostMaxMs exceeds its gate`);
  }
}

positive(budgets.gates.ipc.responseKb, "gates.ipc.responseKb");

exactKeys(budgets.manualTargets, ["nativeStartup", "interaction", "idle", "player"], "manualTargets");
exactKeys(budgets.manualTargets.nativeStartup, ["chromePaintedMsMac", "chromePaintedMsWin"], "manualTargets.nativeStartup");
exactKeys(budgets.manualTargets.interaction, ["sortRepaintMs", "searchKeystrokeP95Ms", "fetchRowsMs"], "manualTargets.interaction");
exactKeys(budgets.manualTargets.idle, ["cpuPercent", "rustRssMb", "webviewRssMb"], "manualTargets.idle");
exactKeys(
  budgets.manualTargets.player,
  ["deckLoadToAudioMs", "scrubToVisibleMs", "deckTickHz", "waveformRedrawsPerSecond"],
  "manualTargets.player",
);
for (const [group, values] of Object.entries(budgets.manualTargets)) {
  for (const [name, value] of Object.entries(values)) positive(value, `manualTargets.${group}.${name}`);
}

exactKeys(budgets.enforcement, ["ci", "local", "measurement"], "enforcement");
exactKeys(budgets.enforcement.ci, ["bundle", "firstRows", "ipcResponse"], "enforcement.ci");
exactKeys(budgets.enforcement.local, ["deckFrames", "scrollFrames"], "enforcement.local");
for (const [scope, values] of Object.entries({ ci: budgets.enforcement.ci, local: budgets.enforcement.local })) {
  for (const [name, value] of Object.entries(values)) {
    if (typeof value !== "string" || value.length === 0) throw new Error(`enforcement.${scope}.${name} must name its check`);
  }
}
if (typeof budgets.enforcement.measurement !== "string" || budgets.enforcement.measurement.length === 0) {
  throw new Error("enforcement.measurement must name the rebaseline command");
}

console.log("Performance budget schema and recorded baseline are valid.");
