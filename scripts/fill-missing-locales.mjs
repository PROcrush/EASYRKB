/**
 * Adds missing src/i18n/ui.json keys to public/locales/*.json using the
 * English string as fallback, retaining existing translations.
 * Run: node scripts/fill-missing-locales.mjs. CI uses this to ensure coverage;
 * rewrites only locale files with missing keys and makes no translation requests.
 */
import { readdirSync, readFileSync, writeFileSync } from "node:fs";

const root = new URL("../", import.meta.url);
const strings = JSON.parse(readFileSync(new URL("src/i18n/ui.json", root), "utf8"));
const locales = new URL("public/locales/", root);
let changed = false;

for (const name of readdirSync(locales).filter((file) => file.endsWith(".json")).sort()) {
  const file = new URL(name, locales);
  const catalog = JSON.parse(readFileSync(file, "utf8"));
  const missing = strings.filter((text) => catalog[text] === undefined);
  if (missing.length === 0) continue;

  for (const text of missing) catalog[text] = text;
  writeFileSync(file, `${JSON.stringify(catalog, null, 2)}\n`);
  console.log(`${name}: added ${missing.length} English fallback${missing.length === 1 ? "" : "s"}`);
  changed = true;
}

if (!changed) console.log("Every locale already has every UI string.");
