import assert from "node:assert/strict";
import test from "node:test";

import { nextStableVersion } from "./next-release-version.mjs";

test("advances a stable version by the largest Conventional Commit change", () => {
  assert.equal(nextStableVersion("1.0.0", [
    { subject: "fix: repair export" },
    { subject: "feat: add library protection warning" },
  ]), "1.1.0");
  assert.equal(nextStableVersion("1.1.0", [
    { subject: "fix: repair export" },
  ]), "1.1.1");
  assert.equal(nextStableVersion("1.1.1", [
    { subject: "feat(api)!: replace export contract" },
  ]), "2.0.0");
});

test("recognizes a breaking-change trailer", () => {
  assert.equal(nextStableVersion("2.4.8", [{
    subject: "refactor: simplify export",
    body: "BREAKING CHANGE: old export profiles are no longer accepted",
  }]), "3.0.0");
});

test("rejects prerelease inputs and empty release ranges", () => {
  assert.throws(() => nextStableVersion("1.0.1-rc.1", [{ subject: "fix: repair export" }]), /stable SemVer/);
  assert.throws(() => nextStableVersion("1.0.0", []), /no commits exist/);
});
