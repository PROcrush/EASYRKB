#!/usr/bin/env node
// Project reviewed GitHub work state to all linked RBX support threads.
// GitHub event runs make it prompt; the bot's periodic pass repairs misses.
import {
  aggregateStatuses, canonicalDuplicateNumber, extractFixPrNumbers,
  extractThreadIds, mergeForumTags, NO_FIX_LABELS, resolveStatus,
  WORKFLOW_LABELS, RBX_FORUM_TAGS,
} from "./support-status.mjs";

const repo = process.env.GITHUB_REPOSITORY;
const githubToken = process.env.GITHUB_TOKEN ?? process.env.GH_TOKEN;
const discordToken = process.env.RBX_DISCORD_BOT_TOKEN;
const guildId = process.env.RBX_DISCORD_GUILD_ID;
const forumIds = new Set((process.env.RBX_SUPPORT_FORUM_IDS ?? "").split(",").map((id) => id.trim()));
const dryRun = process.argv.includes("--dry-run");
const releaseMode = process.argv.includes("--release");
if (!repo || !guildId || !process.env.RBX_MAINTAINER || !process.env.RBX_SUPPORT_FORUM_IDS ||
    !process.env.RBX_UPDATE_FEED || Object.values(RBX_FORUM_TAGS).some(tag => !tag)) {
  throw new Error("Configure your own repository, maintainer, support forum tags and update feed before syncing");
}
if (!githubToken || (!discordToken && !dryRun)) {
  throw new Error("GITHUB_TOKEN and RBX_DISCORD_BOT_TOKEN are required");
}

async function api(base, path, token, init = {}) {
  const response = await fetch(`${base}${path}`, {
    ...init,
    headers: {
      Authorization: base.includes("github") ? `Bearer ${token}` : `Bot ${token}`,
      Accept: base.includes("github") ? "application/vnd.github+json" : "application/json",
      "Content-Type": "application/json",
      ...init.headers,
    },
    signal: AbortSignal.timeout(20_000),
  });
  if (!response.ok) {
    throw new Error(`${base.includes("github") ? "GitHub" : "Discord"} ${init.method ?? "GET"} ${path}: HTTP ${response.status} ${(await response.text()).slice(0, 300)}`);
  }
  const body = await response.text();
  return body ? JSON.parse(body) : null;
}

const gh = (path, init) => api("https://api.github.com", path, githubToken, init);
const discord = (path, init) => api("https://discord.com/api/v10", path, discordToken, init);
const repoPath = `/repos/${repo}`;

async function pages(path, maxPages = 20) {
  const all = [];
  for (let page = 1; page <= maxPages; page++) {
    const chunk = await gh(`${path}${path.includes("?") ? "&" : "?"}per_page=100&page=${page}`);
    all.push(...chunk);
    if (chunk.length < 100) return all;
  }
  throw new Error(`Pagination exceeded ${maxPages} pages: ${path}`);
}

async function mapLimit(items, limit, fn) {
  let cursor = 0;
  const out = new Array(items.length);
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, async () => {
    while (cursor < items.length) {
      const index = cursor++;
      out[index] = await fn(items[index]);
    }
  }));
  return out;
}

const ancestry = new Map();
async function onRef(sha, ref) {
  const key = `${sha}:${ref}`;
  if (!ancestry.has(key)) {
    const comparison = await gh(`${repoPath}/compare/${encodeURIComponent(sha)}...${encodeURIComponent(ref)}`);
    ancestry.set(key, comparison.status === "ahead" || comparison.status === "identical");
  }
  return ancestry.get(key);
}

async function publicRelease() {
  const response = await fetch(process.env.RBX_UPDATE_FEED, {
    signal: AbortSignal.timeout(20_000),
  });
  if (!response.ok) throw new Error(`Public RBX feed returned ${response.status}`);
  const feed = await response.json();
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(feed.version ?? "")) {
    throw new Error("Public RBX feed has no valid version");
  }
  const tag = `v${feed.version}`;
  await gh(`${repoPath}/git/ref/tags/${encodeURIComponent(tag)}`);
  return { version: feed.version, tag };
}

async function fixPrState(numbers, releaseTag) {
  return mapLimit(numbers, 4, async (number) => {
    const pr = await gh(`${repoPath}/pulls/${number}`);
    if (!pr.merged_at || pr.base.ref !== "dev" || !pr.merge_commit_sha) {
      return { number, onDev: false, onMain: false, onPublicRelease: false };
    }
    const sha = pr.merge_commit_sha;
    return {
      number,
      onDev: await onRef(sha, "dev"),
      onMain: await onRef(sha, "main"),
      onPublicRelease: await onRef(sha, releaseTag),
    };
  });
}

async function setStatusLabel(issue, status) {
  if (!status) return false;
  const old = issue.labels.map((label) => label.name);
  const desired = [
    ...old.filter((label) => !WORKFLOW_LABELS.includes(label)),
    `status:${status}`,
  ];
  if (old.length === desired.length && old.every((label, index) => label === desired[index])) {
    return false;
  }
  if (!dryRun) {
    await gh(`${repoPath}/issues/${issue.number}`, {
      method: "PATCH", body: JSON.stringify({ labels: desired }),
    });
  }
  issue.labels = desired.map((name) => ({ name }));
  return true;
}

async function ensureDoneLabel() {
  if (dryRun) return;
  try {
    await gh(`${repoPath}/labels/${encodeURIComponent("status:done")}`);
  } catch (error) {
    if (!error.message.includes("HTTP 404")) throw error;
    await gh(`${repoPath}/labels`, {
      method: "POST",
      body: JSON.stringify({
        name: "status:done",
        color: "0E8A16",
        description: "Verified fix is on both dev and main",
      }),
    });
  }
}

async function updateThreadTags(threadId, status) {
  if (dryRun) return { threadId, status, dryRun: true };
  const thread = await discord(`/channels/${threadId}`);
  if (!forumIds.has(thread.parent_id)) return { threadId, skipped: "not an RBX forum thread" };
  const current = thread.applied_tags ?? [];
  const target = mergeForumTags(current, status);
  if (current.length === target.length && current.every((tag, index) => tag === target[index])) {
    return { threadId, status, changed: false };
  }
  const archived = thread.thread_metadata?.archived ?? false;
  await discord(`/channels/${threadId}`, {
    method: "PATCH",
    body: JSON.stringify(archived ? { applied_tags: target, archived: false } : { applied_tags: target }),
  });
  if (archived) await discord(`/channels/${threadId}`, {
    method: "PATCH", body: JSON.stringify({ archived: true }),
  });
  return { threadId, status, changed: true };
}

async function postTrackedReply(threadId, issue, content, marker, comments) {
  if (comments.some((comment) => comment.body.includes(marker))) return false;
  if (dryRun) return true;
  const thread = await discord(`/channels/${threadId}`);
  if (!forumIds.has(thread.parent_id)) return false;
  const recent = await discord(`/channels/${threadId}/messages?limit=100`);
  if (!recent.some((message) => message.author?.bot && message.content === content)) {
    const archived = thread.thread_metadata?.archived ?? false;
    if (archived) await discord(`/channels/${threadId}`, {
      method: "PATCH", body: JSON.stringify({ archived: false }),
    });
    try {
      await discord(`/channels/${threadId}/messages`, {
        method: "POST", body: JSON.stringify({ content, allowed_mentions: { parse: [] } }),
      });
    } finally {
      if (archived) await discord(`/channels/${threadId}`, {
        method: "PATCH", body: JSON.stringify({ archived: true }),
      });
    }
  }
  await gh(`${repoPath}/issues/${issue.number}/comments`, {
    method: "POST", body: JSON.stringify({ body: marker }),
  });
  return true;
}

async function postReleaseReply(threadId, issue, version, comments) {
  const marker = `<!-- rbx-release-delivered: ${version}:${threadId} -->`;
  const content = `RBXport ${version} is public and includes the fix tracked in GitHub issue #${issue.number}: ${issue.html_url}`;
  return postTrackedReply(threadId, issue, content, marker, comments);
}

async function main() {
  const release = await publicRelease();
  await ensureDoneLabel();
  const issues = (await pages(`${repoPath}/issues?state=all`))
    .filter((issue) => !issue.pull_request);
  const commentsByIssue = new Map();
  await mapLimit(issues.filter((issue) => issue.comments > 0), 8, async (issue) => {
    commentsByIssue.set(issue.number, await pages(`${repoPath}/issues/${issue.number}/comments`));
  });
  const threadIssues = new Map();
  const linksByIssue = new Map();
  const issueByNumber = new Map(issues.map((issue) => [issue.number, issue]));
  const fixStateByIssue = new Map();
  const statusByIssue = new Map();
  const duplicateRedirects = [];
  let labelsChanged = 0;

  for (const issue of issues) {
    const comments = commentsByIssue.get(issue.number) ?? [];
    const texts = [issue.body ?? "", ...comments.map((comment) => comment.body)];
    const directThreads = [...new Set(texts.flatMap((body) => extractThreadIds(body, guildId, forumIds)))];
    if (directThreads.length) linksByIssue.set(issue.number, directThreads);
    const fixNumbers = extractFixPrNumbers(comments);
    const fixStates = fixNumbers.length ? await fixPrState(fixNumbers, release.tag) : [];
    fixStateByIssue.set(issue.number, fixStates);
    const status = resolveStatus(issue, fixStates);
    statusByIssue.set(issue.number, status);
    if (fixStates.length && status && await setStatusLabel(issue, status)) labelsChanged++;
  }

  for (const issue of issues) {
    const comments = commentsByIssue.get(issue.number) ?? [];
    const canonical = canonicalDuplicateNumber(issue, comments);
    const target = canonical && issueByNumber.has(canonical) ? canonical : issue.number;
    for (const threadId of linksByIssue.get(issue.number) ?? []) {
      const linked = threadIssues.get(threadId) ?? new Set();
      linked.add(target);
      threadIssues.set(threadId, linked);
      if (canonical && target !== issue.number) {
        duplicateRedirects.push({ threadId, intake: issue, canonical: issueByNumber.get(target), comments });
      }
    }
  }

  let tagsChanged = 0;
  let releaseReplies = 0;
  let duplicateReplies = 0;
  const errors = [];
  for (const redirect of duplicateRedirects) {
    try {
      const marker = `<!-- rbx-duplicate-redirect: ${redirect.threadId}:${redirect.canonical.number} -->`;
      const content = `This report matches GitHub issue #${redirect.canonical.number}, where the work is tracked: ${redirect.canonical.html_url}`;
      if (await postTrackedReply(redirect.threadId, redirect.intake, content, marker,
        redirect.comments)) duplicateReplies++;
    } catch (error) {
      errors.push(`duplicate redirect ${redirect.threadId}: ${error.message}`);
    }
  }
  for (const [threadId, issueNumbers] of threadIssues) {
    try {
      const work = [...issueNumbers].filter((number) =>
        !issueByNumber.get(number).labels.some((label) => NO_FIX_LABELS.has(label.name)));
      const status = aggregateStatuses(work.map((number) => statusByIssue.get(number) ?? null));
      const updated = await updateThreadTags(threadId, status);
      if (updated.changed) tagsChanged++;
      if (releaseMode) {
        for (const number of work) {
          const issue = issueByNumber.get(number);
          const fixes = fixStateByIssue.get(number) ?? [];
          if (fixes.length && fixes.every((fix) => fix.onPublicRelease)) {
            if (await postReleaseReply(threadId, issue, release.version,
              commentsByIssue.get(number) ?? [])) releaseReplies++;
          }
        }
      }
    } catch (error) {
      errors.push(`thread ${threadId}: ${error.message}`);
    }
  }
  console.log(JSON.stringify({
    issues: issues.length, linkedThreads: threadIssues.size, labelsChanged,
    tagsChanged, duplicateReplies, releaseReplies, releaseVersion: release.version,
    dryRun, errors,
  }));
  if (errors.length) process.exitCode = 1;
}

await main();
