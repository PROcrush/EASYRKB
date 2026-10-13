import { useState } from "react";
import { getBackend } from "@/ipc/client";

export function StopExport({ path, className }: { path: string; className?: string | undefined }) {
  const [stopping, setStopping] = useState(false);
  const [error, setError] = useState("");
  const stop = () => {
    setStopping(true);
    setError("");
    void getBackend().then(backend => backend.cancelExport(path)).catch(() => {
      setStopping(false);
      setError("Could not stop the export. Try again.");
    });
  };
  return <>
    <button type="button" className={className} onClick={stop} disabled={stopping}
      aria-label={`Stop export to ${path}`} title="Stop after the current file operation finishes">
      {stopping ? "Stopping…" : "Stop"}
    </button>
    {error ? <span role="alert">{error}</span> : null}
  </>;
}
