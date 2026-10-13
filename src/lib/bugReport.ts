/** Compose a local report. It is only saved when the user asks to open it. */
export function formatBugReportAttachment(base: string, description: string, preferences: object): string {
  const details = JSON.stringify({ description: description.trim(), preferences }, null, 2);
  return `Report details\n${details}\n\n${base}`;
}
