export const RBX_FORUM_TAGS = Object.freeze({
  "in-progress": process.env.RBX_SUPPORT_TAG_IN_PROGRESS,
  "under-review": process.env.RBX_SUPPORT_TAG_UNDER_REVIEW,
  done: process.env.RBX_SUPPORT_TAG_DONE,
});

export const WORKFLOW_LABELS = [
  "status:in-progress",
  "status:under-review",
  "status:in-review", // retired; remove during a verified transition
  "status:done",
];

export const NO_FIX_LABELS = new Set([
  "duplicate", "invalid", "wontfix", "not-a-fix", "no-announce", "cannot-reproduce",
]);

export function extractThreadIds(text, guildId, forumIds = []) {
  const ids = new Set();
  const forums = new Set(forumIds);
  const sourceUrls = [
    ...[...(text ?? "").matchAll(/## Discord source\s*\n\s*(https:\/\/discord\.com\/channels\/[^\s)]+)/gi)].map((match) => match[1]),
    ...[...(text ?? "").matchAll(/\[Discord source\]\((https:\/\/discord\.com\/channels\/[^)]+)\)/gi)].map((match) => match[1]),
  ];
  for (const url of sourceUrls) {
    const match = new RegExp(`^https://discord\\.com/channels/${guildId}/([0-9]+)(?:/([0-9]+))?(?![0-9/])`).exec(url);
    if (match) ids.add(match[2] && forums.has(match[1]) ? match[2] : match[1]);
  }
  for (const match of (text ?? "").matchAll(/<!-- rbx-support-thread: ([0-9]+) -->/g)) {
    ids.add(match[1]);
  }
  return [...ids];
}

export function extractFixPrNumbers(comments) {
  const trusted = comments.filter((comment) =>
    [process.env.RBX_MAINTAINER, "github-actions[bot]"].filter(Boolean).includes(comment.user?.login));
  const marked = trusted.filter((comment) =>
    comment.body.includes("<!-- rbx-queue-status -->"));
  const latest = marked.at(-1)?.body ?? "";
  const line = /^- \*\*Fix PR:\*\* (.+)$/m.exec(latest)?.[1] ?? "";
  const fromQueue = [...line.matchAll(/(?:\/pull\/|#)([0-9]+)/g)].map((match) => Number(match[1]));
  const explicit = trusted.flatMap((comment) =>
    [...comment.body.matchAll(/<!-- rbx-fix-pr: ([0-9]+) -->/g)].map((match) => Number(match[1])));
  return [...new Set([...fromQueue, ...explicit])];
}

export function canonicalDuplicateNumber(issue, comments) {
  if (!issue.labels.some((label) => label.name === "duplicate")) return null;
  for (const comment of comments) {
    if (![process.env.RBX_MAINTAINER, "github-actions[bot]"].filter(Boolean).includes(comment.user?.login)) continue;
    const match = /Duplicate of #([0-9]+)/i.exec(comment.body);
    if (match) return Number(match[1]);
  }
  return null;
}

export function resolveStatus(issue, fixPrs) {
  const labels = new Set(issue.labels.map((label) => label.name));
  if ([...labels].some((label) => NO_FIX_LABELS.has(label))) return null;
  if (fixPrs.length && fixPrs.every((pr) => pr.onDev && pr.onMain)) return "done";
  if (fixPrs.length && fixPrs.every((pr) => pr.onDev && !pr.onPublicRelease)) {
    return "under-review";
  }
  // Existing queue labels remain the display state until their fix links are
  // audited. They cannot promote themselves to done or create a release reply.
  if (!fixPrs.length && labels.has("status:under-review")) return "under-review";
  if (!fixPrs.length && labels.has("status:done")) return "done";
  if (labels.has("status:in-progress") &&
      issue.assignees.some((assignee) => process.env.RBX_MAINTAINER && assignee.login === process.env.RBX_MAINTAINER)) {
    return "in-progress";
  }
  // Casual PR mentions and issue closure carry no status evidence.
  return null;
}

export function aggregateStatuses(statuses) {
  if (!statuses.length || statuses.includes(null)) return null;
  if (statuses.includes("in-progress")) return "in-progress";
  if (statuses.includes("under-review")) return "under-review";
  return "done";
}

export function mergeForumTags(current, status) {
  const workflow = new Set(Object.values(RBX_FORUM_TAGS));
  const preserved = current.filter((tag) => !workflow.has(tag));
  return status ? [...preserved, RBX_FORUM_TAGS[status]] : preserved;
}
