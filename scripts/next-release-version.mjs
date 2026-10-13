#!/usr/bin/env node

/**
 * Chooses the next stable SemVer from Conventional Commits since the tag for
 * the version currently recorded in Cargo.toml.
 *
 * Run: node scripts/next-release-version.mjs
 */
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const STABLE_VERSION = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const CONVENTIONAL_SUBJECT = /^[a-z][a-z0-9-]*(?:\([^)\r\n]+\))?(!?):/i;
const FEATURE_SUBJECT = /^feat(?:\([^)\r\n]+\))?:/i;

export function nextStableVersion(current, commits) {
  const match = STABLE_VERSION.exec(current);
  if (!match) throw new Error(`workspace version must be stable SemVer; got ${current}`);
  if (!commits.length) throw new Error(`no commits exist after v${current}`);

  let bump = "patch";
  for (const { subject, body = "" } of commits) {
    const conventional = CONVENTIONAL_SUBJECT.exec(subject);
    if (conventional?.[1] === "!" || /^BREAKING CHANGE:\s*\S/m.test(body)) {
      bump = "major";
      break;
    }
    if (FEATURE_SUBJECT.test(subject)) bump = "minor";
  }

  let major = Number(match[1]);
  let minor = Number(match[2]);
  let patch = Number(match[3]);
  if (bump === "major") {
    major += 1;
    minor = 0;
    patch = 0;
  } else if (bump === "minor") {
    minor += 1;
    patch = 0;
  } else {
    patch += 1;
  }
  return `${major}.${minor}.${patch}`;
}

function workspaceVersion() {
  const cargo = readFileSync(new URL("../Cargo.toml", import.meta.url), "utf8");
  const match = /\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m.exec(cargo);
  if (!match) throw new Error("Cargo.toml has no workspace version");
  return match[1];
}

function commitsSince(tag) {
  const git = args => execFileSync("git", args, { cwd: root, encoding: "utf8" });
  git(["rev-parse", "--verify", `refs/tags/${tag}^{commit}`]);
  git(["merge-base", "--is-ancestor", tag, "HEAD"]);
  return git(["log", "--format=%s%x1f%b%x1e", `${tag}..HEAD`])
    .split("\x1e")
    .filter(entry => entry.trim())
    .map(entry => {
      const [subject = "", body = ""] = entry.trim().split("\x1f");
      return { subject, body };
    });
}

function main() {
  const current = workspaceVersion();
  process.stdout.write(nextStableVersion(current, commitsSince(`v${current}`)));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    main();
  } catch (error) {
    console.error(`next-release-version: ${error.message}`);
    process.exitCode = 1;
  }
}
