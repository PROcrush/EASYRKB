/**
 * The header's right-click menu: auto-size, then every column with a tick
 * beside the ones on show.
 *
 * Transcribed from rekordbox's own menu, so the order is the catalogue's and
 * the two auto-size actions sit above a separator at the top.
 */
import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";

import { MENU_COLUMNS, REQUIRED_COLUMNS, type ColumnKey } from "@/lib/columns";
import styles from "./ColumnMenu.module.css";

/** Matches `.menu`'s width, for keeping the menu inside the window. */
const MENU_WIDTH = 190;
/** Two commands, a divider and the compact column rows. */
const FULL_HEIGHT = 2 * 25 + MENU_COLUMNS.length * 20 + 16;

export interface ColumnMenuProps {
  x: number;
  y: number;
  /** The column the menu was opened on, for "Auto-size this column". */
  target: ColumnKey | null;
  visible: readonly ColumnKey[];
  onToggle: (key: ColumnKey) => void;
  onAutoSize: (key: ColumnKey) => void;
  onAutoSizeAll: () => void;
  onClose: () => void;
}

export function ColumnMenu({
  x, y, target, visible, onToggle, onAutoSize, onAutoSizeAll, onClose,
}: ColumnMenuProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Any click outside, any scroll, or Escape closes it — a menu that
    // outlives the thing it was opened on is worse than no menu.
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("mousedown", onDown, true);
    window.addEventListener("keydown", onKey);
    window.addEventListener("resize", onClose);
    return () => {
      window.removeEventListener("mousedown", onDown, true);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose]);

  const shown = new Set(visible);
  const top = Math.max(8, Math.min(y, window.innerHeight - FULL_HEIGHT - 8));

  return createPortal(
    <div
      ref={ref}
      className={styles.menu}
      style={{
        left: Math.max(8, Math.min(x, window.innerWidth - MENU_WIDTH - 8)),
        top,
        maxHeight: window.innerHeight - top - 8,
      }}
      role="menu"
      aria-label="Columns"
    >
      <button
        type="button"
        className={styles.item}
        role="menuitem"
        disabled={target === null}
        onClick={() => {
          if (target) onAutoSize(target);
          onClose();
        }}
      >
        Auto-size this column
      </button>
      <button
        type="button"
        className={styles.item}
        role="menuitem"
        onClick={() => {
          onAutoSizeAll();
          onClose();
        }}
      >
        Auto-size all columns
      </button>
      <div className={styles.separator} role="separator" />
      <div className={styles.list}>
        {MENU_COLUMNS.map((column) => (
          <button
            key={column.key}
            type="button"
            className={styles.item}
            role="menuitemcheckbox"
            aria-checked={shown.has(column.key)}
            disabled={REQUIRED_COLUMNS.includes(column.key)}
            onClick={() => onToggle(column.key)}
          >
            <span className={styles.tick} aria-hidden>
              {shown.has(column.key) ? "✓" : ""}
            </span>
            {column.label}
          </button>
        ))}
      </div>
    </div>,
    document.body,
  );
}
