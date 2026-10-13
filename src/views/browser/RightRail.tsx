/**
 * The icon column at the right edge of the browser.
 *
 * Two outlined boxes: Information and Sub-Browser. rekordbox draws five —
 * My Tag, Related Tracks and Track Suggestion above these — and those three
 * are left out of this column by request, recorded in TODO.md under
 * "Deliberate divergences". Related Tracks is hidden until finished,
 * and My Tag filtering is the Track Filter's. The two remaining buttons
 * are vertically centered as a group. The wording is german.lang's.
 */
import { InfoIcon, SubBrowseIcon } from "@/components/icons";
import styles from "./RightRail.module.css";
import { useTooltip } from "@/store/usePreferences";

export interface RightRailProps {
  /** Where the shell puts it, since the shell's grid decides that. */
  className?: string | undefined;
  infoOpen: boolean;
  onToggleInfo: () => void;
  subOpen: boolean;
  onToggleSub: () => void;
}

export function RightRail({ className, infoOpen, onToggleInfo, subOpen, onToggleSub }: RightRailProps) {
  const tip = useTooltip();
  return (
    <div className={className ? `${styles.rail} ${className}` : styles.rail} role="toolbar" aria-label="Browser panels" aria-orientation="vertical">
      <div className={styles.group}>
        <button
          type="button"
          className={styles.button}
          title={tip("Display information window.")}
          aria-label="Information"
          aria-pressed={infoOpen}
          onClick={onToggleInfo}
        >
          <InfoIcon className={styles.icon} />
        </button>
        <button
          type="button"
          className={styles.button}
          title={tip("Display sub-browser window.")}
          aria-label="Sub-Browser Window"
          aria-pressed={subOpen}
          onClick={onToggleSub}
        >
          <SubBrowseIcon className={styles.icon} />
        </button>
      </div>
    </div>
  );
}
