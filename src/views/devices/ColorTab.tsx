/**
 * Color: "Customize Color Comments" — eight rows, a dot and an editable name
 * each, Pink to Purple in rekordbox's order. Capture: docs/screenshots
 * 9.08.41 PM.
 *
 * The names are `color.name` in `exportLibrary.db`, written on Enter or
 * blur. The dots are the colours measured off the capture, by position: the
 * stick stores no colour value, only the eight names in a fixed order.
 */
import { useEffect, useState } from "react";

import type { ColorName, DeviceSettings } from "@/ipc/types";
import styles from "./DevicePanel.module.css";

/** The dot for each `color_id`, 1 to 8, as `--c-label-*` tokens. */
const DOTS = ["pink", "red", "orange", "yellow", "green", "aqua", "blue", "purple"] as const;

interface RowProps {
  color: ColorName;
  disabled: boolean;
  onRename: (name: string) => void;
}

function ColorRow({ color, disabled, onRename }: RowProps) {
  const [draft, setDraft] = useState(color.name);
  useEffect(() => setDraft(color.name), [color.name]);
  const dot = DOTS[color.id - 1] ?? "pink";
  const commit = () => {
    const trimmed = draft.trim();
    if (trimmed === "" ) {
      setDraft(color.name);
      return;
    }
    if (trimmed !== color.name) onRename(trimmed);
  };
  return (
    <label className={styles.colorRow}>
      <span className={styles.colorDot} data-color={dot} aria-hidden />
      <input
        className={styles.colorName}
        value={draft}
        aria-label={`Color comment ${color.id}`}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
        }}
        disabled={disabled}
        maxLength={64}
      />
    </label>
  );
}

export interface ColorTabProps {
  settings: DeviceSettings;
  onChange: (next: DeviceSettings) => void;
}

export function ColorTab({ settings, onChange }: ColorTabProps) {
  return (
    <div className={styles.colors}>
      <span className={styles.caption}>Customize Color Comments</span>
      {settings.colors.map((color) => (
        <ColorRow
          key={color.id}
          color={color}
          disabled={!settings.hasLibrarySettings}
          onRename={(name) =>
            onChange({
              ...settings,
              colors: settings.colors.map((c) => (c.id === color.id ? { ...c, name } : c)),
            })
          }
        />
      ))}
    </div>
  );
}
