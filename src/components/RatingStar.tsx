/**
 * One star of a rating, lit or not.
 *
 * Drawn, not typed: none of the fonts in our stack has ★ or ☆, so each
 * platform used to substitute its own and the stars came out a different size
 * everywhere. The caller's class sets the height; the width follows the
 * glyph's proportions, and the colour comes from `currentColor`.
 */
import { StarEmptyIcon, StarLitIcon } from "./icons";
import styles from "./RatingStar.module.css";

export function RatingStar({ lit, className }: { lit: boolean; className?: string | undefined }) {
  const Icon = lit ? StarLitIcon : StarEmptyIcon;
  return <Icon className={className ? `${styles.star} ${className}` : styles.star} data-lit={lit || undefined} />;
}
