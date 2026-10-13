/**
 * How much of the window the deck takes.
 *
 * rekordbox puts this in the top left corner, and it is a real layout switch
 * rather than a zoom: the browser is the same browser in every one of them,
 * and what changes is how many decks sit above it and how much of a deck each
 * of those shows.
 */
export type PlayerLayout = "one" | "two" | "simple" | "browser";

export const LAYOUTS: readonly { id: PlayerLayout; label: string }[] = [
  { id: "one", label: "1 PLAYER" },
  { id: "two", label: "2 PLAYER" },
  { id: "simple", label: "SIMPLE PLAYER" },
  { id: "browser", label: "FULL BROWSER" },
];

/** How many decks a layout draws. */
export function deckCount(layout: PlayerLayout): number {
  if (layout === "browser") return 0;
  if (layout === "two") return 2;
  return 1;
}

/**
 * Whether a layout draws the whole deck.
 *
 * The simple player is one strip: PLAY, the sleeve, the readouts over the
 * overview waveform, the rating — the parts you read a track with, without
 * the detail waveform, the transport rail and the cue list you set it up with.
 */
export function isFullDeck(layout: PlayerLayout): boolean {
  return layout !== "simple";
}

/** The label the dropdown shows, falling back rather than going blank. */
export function layoutLabel(layout: PlayerLayout): string {
  return LAYOUTS.find((entry) => entry.id === layout)?.label ?? "1 PLAYER";
}

/** Reads a stored or wire value back, so an unknown one cannot wedge the app. */
export function asLayout(value: unknown): PlayerLayout {
  return LAYOUTS.some((entry) => entry.id === value) ? (value as PlayerLayout) : "one";
}
