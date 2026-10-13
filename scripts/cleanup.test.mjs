/**
 * Tests cleanup argument parsing, deletion planning, preservation of user
 * data, and merged-branch eligibility using temporary fixtures.
 * Run: pnpm exec vitest run scripts/cleanup.test.mjs (also included in pnpm test).
 */
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import { cleanupPlan, mergedBranchCandidates, parseArguments } from "./cleanup.mjs";

const temporary = [];

afterEach(async () => {
  const { rm } = await import("node:fs/promises");
  await Promise.all(temporary.splice(0).map(directory => rm(directory, { recursive: true, force: true })));
});

async function fixture() {
  const root = await mkdtemp(path.join(tmpdir(), "rbxport-cleanup-"));
  temporary.push(root);
  const repository = path.join(root, "repository");
  const state = path.join(root, "state");
  const destination = path.join(root, "destination");
  await Promise.all([mkdir(repository), mkdir(state), mkdir(destination)]);
  return { repository, state, destination, caches: [path.join(root, "cache")] };
}

describe("cleanup script", () => {
  it("defaults to generated checkout output", async () => {
    const options = parseArguments([]);
    const setup = await fixture();
    const plan = await cleanupPlan(options, setup.repository, setup);

    expect(plan.map(item => path.relative(setup.repository, item.path))).toEqual([
      "target",
      "dist",
      "test-results",
      "playwright-report",
      "dist-mock",
    ]);
    expect(plan.every(item => item.path.startsWith(`${setup.repository}${path.sep}`))).toBe(true);
  });

  it("adds dependencies only when explicitly requested", async () => {
    const setup = await fixture();
    const plan = await cleanupPlan(parseArguments(["--dependencies"]), setup.repository, setup);
    expect(plan.at(-1)).toEqual({ kind: "dependencies", path: path.join(setup.repository, "node_modules") });
  });

  it("selects only partial work in the state and configured backup folders", async () => {
    const setup = await fixture();
    await writeFile(path.join(setup.state, "backup-destination.json"), JSON.stringify(setup.destination));
    await mkdir(path.join(setup.state, ".partial-11111111-1111-1111-1111-111111111111"));
    await writeFile(path.join(setup.destination, ".partial-22222222-2222-2222-2222-222222222222.zip"), "partial");
    await writeFile(path.join(setup.destination, "rbexport-2026-10-04.zip"), "keep");
    await writeFile(path.join(setup.state, "undo-history.json"), "keep");

    const plan = await cleanupPlan(parseArguments(["--app-data"]), setup.repository, setup);
    const appPaths = plan.filter(item => item.kind !== "generated output").map(item => item.path);

    expect(appPaths).toEqual([
      setup.caches[0],
      path.join(setup.state, ".partial-11111111-1111-1111-1111-111111111111"),
      path.join(setup.destination, ".partial-22222222-2222-2222-2222-222222222222.zip"),
    ]);
    expect(await readFile(path.join(setup.destination, "rbexport-2026-10-04.zip"), "utf8")).toBe("keep");
    expect(await readFile(path.join(setup.state, "undo-history.json"), "utf8")).toBe("keep");
  });

  it("rejects unknown options", () => {
    expect(() => parseArguments(["--everything"])).toThrow("Unknown option: --everything");
  });

  it("accepts pnpm's optional argument separator", () => {
    expect(parseArguments(["--", "--dry-run"])).toMatchObject({ dryRun: true });
  });

  it("selects merged branches while protecting long-lived and checked-out branches", () => {
    expect(mergedBranchCandidates(
      ["dev", "main", "feature/done", "worktree/done", "feature/open"],
      ["worktree/done"],
      "dev",
    )).toEqual(["feature/done", "feature/open"]);
  });
});
