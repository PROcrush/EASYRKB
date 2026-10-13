/** @vitest-environment jsdom */
import { act, useCallback, useRef, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { __setBackend } from "@/ipc/client";
import { createMockBackend } from "@/ipc/backend-mock";
import type { RowDto } from "@/ipc/types";
import { DEFAULT_PREFERENCES } from "@/lib/preferences";
import { OVERDRAW, type BeatGrid } from "@/lib/player";
import type { Deck } from "@/lib/sync";
import * as sync from "@/lib/sync";
import { PreferencesProvider } from "@/store/usePreferences";
import type { Playback } from "@/store/usePlayback";

const state = vi.hoisted(() => ({ playback: null as unknown as Playback, grid: null as unknown as BeatGrid,
  decks: null as Record<string, Playback> | null }));
vi.mock("@/store/usePlayback", () => ({ usePlayback: (_track: string | null, deck: string) => state.decks?.[deck] ?? state.playback }));
vi.mock("@/store/useElementSize", () => ({
  useElementSize: () => [useCallback(() => {}, []), { width: 600, height: 80 }],
}));
vi.mock("./useTrackGrid", () => ({
  useTrackGrid: (track: { id: string } | null) => ({ grid: state.grid, state: null, setState: () => {}, gridTrackId: track?.id ?? null }),
}));
// Inspect the real player's drawing window; rendering pixels is covered by
// WaveformDetail's tests. All controls, phase timers and scrolling stay real.
vi.mock("./WaveformDetail", () => ({
  WaveformDetail: ({ detail, span, durationMs }: { detail: boolean; span: number; durationMs: number }) =>
    <div data-testid={detail ? "draw-detail" : "draw-overview"} data-span={span} data-duration={durationMs} />,
}));

import { Player, type PlayerProps } from "@/views/player/Player";

const changing: BeatGrid = {
  times: Uint32Array.from([0, 500, 1100, 1800, 2600, 3400]),
  numbers: Uint8Array.from([1, 2, 3, 4, 1, 2]),
  tempos: Uint16Array.from([12000, 10000, 8571, 7500, 7500, 7500]),
};
const track = { id: "sync-fixture", title: "Sync fixture", artist: "Fixture", analysed: 1, bpmX100: 12000,
  durationSec: 180, key: "Am", rating: 0 } as RowDto;
const leader: Deck = { bpmX100: 12000, tempo: 1, playing: true, position: 4.1,
  grid: { times: Uint32Array.from([0, 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000]),
    numbers: Uint8Array.from([1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3]), tempos: new Uint16Array(11).fill(12000) } };
let host: HTMLDivElement;
let root: Root;
let subscribers: Set<(seconds: number) => void>;

beforeEach(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  vi.useFakeTimers();
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
  __setBackend(createMockBackend({ trackCount: 2 }));
  subscribers = new Set();
  const positionRef = { current: 0 };
  state.grid = changing;
  state.decks = null;
  state.playback = {
    playing: false, position: 0, duration: 180, idle: false, error: null, tempo: 1,
    masterTempo: false, keyShift: 0, shiftsKey: true, loop: null, positionRef,
    positionNow: () => positionRef.current,
    subscribe: fn => { subscribers.add(fn); return () => { subscribers.delete(fn); }; },
    toggle: vi.fn(), playAfter: vi.fn(), seek: vi.fn(), moveBy: vi.fn(), seekFraction: vi.fn(),
    setLoop: vi.fn(), setLoopActive: vi.fn(), clearLoop: vi.fn(), scrubBegin: vi.fn(), scrubTo: vi.fn(),
    scrubEnd: vi.fn(), isScrubbing: () => false, setKeyShift: vi.fn(), setTempo: vi.fn(),
    nudgeTempo: vi.fn(), setMasterTempo: vi.fn(),
  };
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
  __setBackend(null);
  vi.useRealTimers();
  vi.restoreAllMocks();
});
async function render(props: Partial<PlayerProps> = {}, syncType: "beat" | "bpm" = "beat") {
  const preferences = { ...DEFAULT_PREFERENCES, advanced: { ...DEFAULT_PREFERENCES.advanced, syncType } };
  await act(async () => {
    root.render(<PreferencesProvider value={{ preferences, update: () => {}, reset: () => {} }}>
      <Player track={track} dual bars={8} {...props} />
    </PreferencesProvider>);
    await Promise.resolve();
  });
}
function frame(seconds: number) {
  act(() => {
    state.playback.positionRef.current = seconds;
    subscribers.forEach(fn => fn(seconds));
  });
}
function drawnSeconds() {
  const draw = host.querySelector('[data-testid="draw-detail"]')!;
  return Number(draw.getAttribute("data-span")) * Number(draw.getAttribute("data-duration")) / 1000 / OVERDRAW;
}

it("converts the common waveform time scale using the deck's playback speed", async () => {
  state.playback.tempo = 1.2;
  await render({ waveformBpmX100: 12000 });
  expect(drawnSeconds()).toBeCloseTo(19.2, 6);
  // Each real second moves by the same amount at every zoom level.
  frame(1.2);
  const x = Number(/translateX\(([-.\d]+)px\)/.exec(host.querySelector('[data-testid="player-detail"] > div')!.getAttribute("style") ?? "")?.[1]);
  frame(2.4);
  const nextX = Number(/translateX\(([-.\d]+)px\)/.exec(host.querySelector('[data-testid="player-detail"] > div')!.getAttribute("style") ?? "")?.[1]);
  expect(x - nextX).toBeCloseTo(600 / 16, 6);
});
it("reports the grid tempo at the head and follows it through a tempo boundary", async () => {
  const report = vi.fn();
  await render({ onPlayingBpm: report, synced: true, peerSync: () => leader, leaderBpmX100: 12000 });
  expect(report).toHaveBeenLastCalledWith(12000);
  frame(1.4);
  expect(report).toHaveBeenLastCalledWith(8571);
  expect(state.playback.setTempo).toHaveBeenLastCalledWith(12000 / 8571);
  expect(host.querySelector('[data-testid="player-bpm"]')?.textContent).toContain("85.71");
});
it("reads the master's precise playback speed instead of its rounded display BPM", async () => {
  state.grid = leader.grid;
  const tempo = 1.234567;
  await render({ synced: true, peerSync: () => ({ ...leader, tempo }), leaderBpmX100: Math.round(12000 * tempo) }, "bpm");
  expect(state.playback.setTempo).toHaveBeenCalledExactlyOnceWith(tempo);
});
it("reports exact output BPM while keeping the displayed BPM rounded", async () => {
  state.grid = leader.grid;
  state.playback.tempo = 1.234567;
  const report = vi.fn();
  await render({ onPlayingBpm: report });
  expect(report).toHaveBeenLastCalledWith(12000 * state.playback.tempo);
  expect(host.querySelector('[data-testid="player-bpm"]')?.textContent).toContain("148.15");
});
it("reads both current heads when the decks cross the same tempo boundary before the shell updates", async () => {
  let head = 0;
  const peerSync = () => ({ ...leader, grid: changing, position: head });
  await render({ synced: true, peerSync, leaderBpmX100: 12000 }, "bpm");
  expect(state.playback.setTempo).not.toHaveBeenCalled();
  head = 1.4;
  frame(head);
  // Both now play at 85.71 BPM; the shell still holds the old 120.00.
  expect(state.playback.setTempo).not.toHaveBeenCalled();
});
it("keeps two real Player components at unity through a shared variable-tempo boundary", async () => {
  const masterSubscribers = new Set<(seconds: number) => void>();
  const masterHead = { current: 0 };
  const master: Playback = { ...state.playback, positionRef: masterHead,
    positionNow: () => masterHead.current, setTempo: vi.fn(),
    subscribe: fn => { masterSubscribers.add(fn); return () => { masterSubscribers.delete(fn); }; } };
  state.decks = { a: master, b: state.playback };
  function Pair() {
    const readMaster = useRef<() => Deck | null>(() => null);
    const peerSync = useCallback(() => readMaster.current(), []);
    const [reportedBpm, setReportedBpm] = useState<number | null>(12000);
    return <>
      <Player track={{ ...track, id: "master-fixture" }} deck="a" dual isMaster
        publishSync={get => { readMaster.current = get; }} onPlayingBpm={setReportedBpm} />
      <Player track={track} deck="b" dual synced peerSync={peerSync} leaderBpmX100={reportedBpm} />
    </>;
  }
  const preferences = { ...DEFAULT_PREFERENCES, advanced: { ...DEFAULT_PREFERENCES.advanced, syncType: "bpm" as const } };
  await act(async () => {
    root.render(<PreferencesProvider value={{ preferences, update: () => {}, reset: () => {} }}><Pair /></PreferencesProvider>);
    await vi.advanceTimersByTimeAsync(0);
  });
  await act(async () => {
    masterHead.current = 1.4;
    state.playback.positionRef.current = 1.4;
    masterSubscribers.forEach(fn => fn(1.4));
    subscribers.forEach(fn => fn(1.4));
    await vi.advanceTimersByTimeAsync(0);
  });
  expect([...host.querySelectorAll('[data-testid="player-bpm"]')].map(node => node.textContent)).toEqual(["85.71", "85.71"]);
  expect(master.setTempo).not.toHaveBeenCalled();
  expect(state.playback.setTempo).not.toHaveBeenCalled();
});
it("does not re-read the Sync pair for unrelated playback state changes", async () => {
  state.grid = leader.grid;
  const peerSync = vi.fn(() => leader);
  const computeTempo = vi.spyOn(sync, "tempoFor");
  const props = { synced: true, peerSync, leaderBpmX100: 12000 };
  await render(props, "bpm");
  expect(peerSync).toHaveBeenCalled();
  peerSync.mockClear();
  computeTempo.mockClear();
  state.playback = { ...state.playback, keyShift: 2, duration: 200, error: "fixture", loop: { inSeconds: 1, outSeconds: 2, active: true } };
  await render(props, "bpm");
  expect(peerSync).not.toHaveBeenCalled();
  expect(computeTempo).not.toHaveBeenCalled();
  expect(state.playback.setTempo).not.toHaveBeenCalled();
});
it("ignores a stale display BPM after the master has been ejected", async () => {
  await render({ synced: true, peerSync: () => null, leaderBpmX100: 14000 }, "bpm");
  expect(state.playback.setTempo).not.toHaveBeenCalled();
});
it("BPM sync changes speed without phase correction even with Quantize on", async () => {
  state.playback.playing = true;
  state.playback.positionRef.current = 4;
  state.grid = leader.grid;
  await render({ synced: true, peerSync: () => leader, leaderBpmX100: 12000 }, "bpm");
  await act(async () => { await vi.advanceTimersByTimeAsync(600); });
  expect(state.playback.moveBy).not.toHaveBeenCalled();
  expect(state.playback.seek).not.toHaveBeenCalled();
});
it("Beat sync with Quantize corrects drift through the engine's current position", async () => {
  state.playback.playing = true;
  state.playback.positionRef.current = 4;
  state.grid = leader.grid;
  await render({ synced: true, peerSync: () => leader, leaderBpmX100: 12000 });
  await act(async () => { await vi.advanceTimersByTimeAsync(100); });
  expect(state.playback.moveBy).toHaveBeenCalledWith(expect.closeTo(0.1, 6));
  expect(state.playback.seek).not.toHaveBeenCalled();
});
it("pressing Sync on a running deck moves relative to the engine instead of seeking to an old UI timestamp", async () => {
  state.playback.playing = true;
  state.playback.positionRef.current = 4;
  state.grid = leader.grid;
  await render({ peerSync: () => leader, onSyncToggle: vi.fn() });
  act(() => (host.querySelector('[aria-label="Beat sync"]') as HTMLButtonElement).click());
  expect(state.playback.moveBy).toHaveBeenCalledWith(expect.closeTo(0.1, 6));
  expect(state.playback.seek).not.toHaveBeenCalled();
});
