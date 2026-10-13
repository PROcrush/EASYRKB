/**
 * What the app is costing, in the title bar's corner.
 *
 * Its own component so that a reading re-renders these three figures and
 * nothing else: as a value of the shell it re-rendered the whole window
 * every time, which was most of the webview's idle load.
 *
 * A dash is a figure the platform will not give rather than a zero, which
 * would be a claim. GPU is not among them: macOS accounts it per process
 * only to root, so it was always a dash.
 */
import { formatCount, formatMemory, formatPercent, useAppCost } from "@/store/useDiagnostics";
import { useTooltip } from "@/store/usePreferences";

export function AppCost({ className }: { className?: string | undefined }) {
  const cost = useAppCost();
  const tip = useTooltip();
  return (
    <div className={className} data-testid="app-cost">
      <span title={tip(`Audio callback deadline usage; ${cost.audioXruns} overruns`)}>AUDIO {formatPercent(cost.audioLoad * 100)}</span>
      <span title={tip("Resident memory")}>RAM {formatMemory(cost.memoryMb)}</span>
      <span title={tip("Frames a second, timed in the window")}>FPS {formatCount(cost.fps)}</span>
    </div>
  );
}
