/** Two animation frames leave a paint opportunity after React commits.
 * Native elapsed time includes process/webview startup and IPC delivery;
 * this is a paint proxy, not a measurement of the display compositor.
 */
const reported = new Set<string>();
export function reportStartupPaint(phase: "shell-painted" | "first-rows-painted"): void {
  if (reported.has(phase)) return;
  reported.add(phase);
  requestAnimationFrame(() => requestAnimationFrame(() => {
    performance.mark(`startup:${phase}`);
    if (!("__TAURI_INTERNALS__" in window)) return;
    void import("@tauri-apps/api/core")
      .then(({ invoke }) => invoke("startup_milestone", { phase }))
      .catch(() => { /* Diagnostics must never affect startup. */ });
  }));
}
