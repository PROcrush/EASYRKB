/**
 * Whether this build can drive a deck.
 *
 * Under Tauri that is the Rust engine behind the `deck_*` commands, which
 * makes sound. In a browser — `pnpm dev:mock`, and Playwright — it is the mock
 * backend's deck, which keeps time and emits the same ticks but is silent.
 * Either way the transport is live: a disabled transport meant the scrolling
 * waveform, the cue point and the readouts could only ever be tested by
 * looking at their markup.
 */
export const canPlay = true;
