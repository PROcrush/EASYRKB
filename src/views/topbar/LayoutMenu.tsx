/**
 * The layout switch, in the corner rekordbox keeps it.
 *
 * Transcribed from a capture of the menu open: a glyph and a chevron on the
 * bar, and a bordered panel of rows, each a tick column, the layout's own
 * icon and its name in capitals. Not a `<select>` — the platform's own control
 * draws none of that and cannot show the icons the menu is mostly made of.
 */
import { useEffect, useRef, useState } from "react";

import {
  LayoutBrowserIcon,
  LayoutOneIcon,
  LayoutSimpleIcon,
  LayoutTwoIcon,
  TickIcon,
} from "@/components/icons";
import { LAYOUTS, type PlayerLayout } from "@/lib/layout";
import styles from "./LayoutMenu.module.css";

const GLYPHS: Record<PlayerLayout, typeof LayoutOneIcon> = {
  one: LayoutOneIcon,
  two: LayoutTwoIcon,
  simple: LayoutSimpleIcon,
  browser: LayoutBrowserIcon,
};

export interface LayoutMenuProps {
  layout: PlayerLayout;
  onChange?: ((layout: PlayerLayout) => void) | undefined;
}

export function LayoutMenu({ layout, onChange }: LayoutMenuProps) {
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  const Current = GLYPHS[layout];

  useEffect(() => {
    if (!open) return;
    // Anything outside closes it, as every other menu here does: one that
    // outlives the thing it was opened on is worse than no menu.
    const onDown = (event: MouseEvent) => {
      if (!box.current?.contains(event.target as Node)) setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("mousedown", onDown, true);
    window.addEventListener("keydown", onKey);
    window.addEventListener("resize", () => setOpen(false));
    return () => {
      window.removeEventListener("mousedown", onDown, true);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div ref={box} className={styles.box}>
      <button
        type="button"
        className={styles.trigger}
        aria-label="Layout"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((was) => !was)}
      >
        <Current className={styles.glyph} />
        <span className={styles.chevron} aria-hidden />
      </button>

      {open ? (
        <div className={styles.menu} role="menu" aria-label="Layout">
          {LAYOUTS.map((entry) => {
            const Glyph = GLYPHS[entry.id];
            return (
              <button
                key={entry.id}
                type="button"
                role="menuitemradio"
                aria-checked={entry.id === layout}
                className={styles.item}
                onClick={() => {
                  onChange?.(entry.id);
                  setOpen(false);
                }}
              >
                <span className={styles.tick} aria-hidden>
                  {entry.id === layout ? <TickIcon className={styles.tickGlyph} /> : null}
                </span>
                <Glyph className={styles.itemGlyph} />
                <span className={styles.label}>{entry.label}</span>
              </button>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}
