/**
 * The beat-jump size menu: every size rekordbox offers, ticked at the one in
 * use.
 *
 * The control it hangs off used to be a cycle — one press, one size along —
 * which meant six presses to get from Fine to 32Bars and no way to see what
 * the sizes were without walking them.
 */
import { useEffect, useRef } from "react";

import { JUMP_SIZES } from "@/lib/player";
import styles from "./JumpMenu.module.css";

/** Matches `.menu`'s width, for keeping it inside the window. */
const MENU_WIDTH = 132;
/** Roughly the list's height, so a menu near the bottom is raised instead. */
const MENU_HEIGHT = 7 * 30 + 8;

export interface JumpMenuProps {
  /** Where the button is, in client coordinates: the menu opens beside it. */
  x: number;
  y: number;
  current: string;
  /**
   * The button the menu hangs off.
   *
   * Excluded from the close-on-outside-click, or pressing it to dismiss the
   * menu closed it on the way down and its own toggle reopened it on the way
   * up, and the menu could only ever be shut by pressing somewhere else.
   */
  anchor: HTMLElement | null;
  onPick: (id: string) => void;
  onClose: () => void;
}

export function JumpMenu({ x, y, current, anchor, onPick, onClose }: JumpMenuProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Any click outside, Escape, or the window changing shape closes it: a
    // menu that outlives what it was opened on is worse than no menu.
    const onDown = (e: MouseEvent) => {
      const at = e.target as Node;
      if (!ref.current?.contains(at) && anchor?.contains(at) !== true) onClose();
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
  }, [onClose, anchor]);

  return (
    <div
      ref={ref}
      className={styles.menu}
      style={{
        left: Math.max(8, x + MENU_WIDTH <= window.innerWidth - 8
          ? x
          : (anchor?.getBoundingClientRect().left ?? x) - MENU_WIDTH - 6),
        top: Math.min(y, Math.max(8, window.innerHeight - MENU_HEIGHT - 8)),
      }}
      role="menu"
      aria-label="Beat jump size"
    >
      {JUMP_SIZES.map((size) => (
        <button
          key={size.id}
          type="button"
          className={styles.item}
          role="menuitemradio"
          aria-checked={size.id === current}
          onClick={() => {
            onPick(size.id);
            onClose();
          }}
        >
          <span className={styles.tick} aria-hidden>
            {size.id === current ? "✓" : ""}
          </span>
          {size.label}
        </button>
      ))}
    </div>
  );
}
