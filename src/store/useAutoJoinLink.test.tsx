// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import type { LinkPeerSeen, LinkStatus } from "@/ipc/types";
import { useAutoJoinLink } from "./useAutoJoinLink";

const player: LinkPeerSeen = { number: 1, name: "CDJ-3000", address: "169.254.1.2", kind: "player" };
const rekordbox: LinkPeerSeen = { number: 17, name: "rekordbox", address: "169.254.1.3", kind: "rekordbox" };
const off: LinkStatus = { on: false, problem: null, interface: null, interfaces: [], players: [], master: false, masterBpm: 120, state: "off", number: null };
const on: LinkStatus = { ...off, on: true, state: "up" };

function Probe({ enabled, peers, status, busy, start }: {
  enabled: boolean;
  peers: LinkPeerSeen[];
  status: LinkStatus | null;
  busy: boolean;
  start: () => Promise<unknown>;
}) {
  useAutoJoinLink(enabled, peers, status, busy, start);
  return null;
}

let host: HTMLDivElement;
let root: Root;
beforeEach(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  host = document.createElement("div");
  root = createRoot(host);
});
afterEach(() => act(() => root.unmount()));

it("joins once when opted in and a player becomes available", async () => {
  const start = vi.fn().mockResolvedValue(on);
  await act(async () => { root.render(<Probe enabled peers={[player]} status={off} busy={false} start={start} />); await Promise.resolve(); });
  expect(start).toHaveBeenCalledTimes(1);
  await act(async () => { root.render(<Probe enabled peers={[player]} status={off} busy={false} start={start} />); await Promise.resolve(); });
  expect(start).toHaveBeenCalledTimes(1);
});

it("does not join by default or for another rekordbox instance", async () => {
  const start = vi.fn().mockResolvedValue(on);
  await act(async () => { root.render(<Probe enabled={false} peers={[player]} status={off} busy={false} start={start} />); await Promise.resolve(); });
  await act(async () => { root.render(<Probe enabled peers={[rekordbox]} status={off} busy={false} start={start} />); await Promise.resolve(); });
  expect(start).not.toHaveBeenCalled();
});

it("allows a manual disconnect until the devices leave and return", async () => {
  const start = vi.fn().mockResolvedValue(on);
  await act(async () => { root.render(<Probe enabled peers={[player]} status={on} busy={false} start={start} />); await Promise.resolve(); });
  await act(async () => { root.render(<Probe enabled peers={[player]} status={off} busy={false} start={start} />); await Promise.resolve(); });
  expect(start).not.toHaveBeenCalled();
  await act(async () => { root.render(<Probe enabled peers={[]} status={off} busy={false} start={start} />); await Promise.resolve(); });
  await act(async () => { root.render(<Probe enabled peers={[player]} status={off} busy={false} start={start} />); await Promise.resolve(); });
  expect(start).toHaveBeenCalledTimes(1);
});
