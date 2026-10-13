// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getBackend } from "@/ipc/client";
import { syncRekordboxBrowseAtStartup } from "./rekordboxBrowse";

vi.mock("@/ipc/client", () => ({ getBackend: vi.fn() }));

const xml = `<PROPERTIES>
  <BROWSELAYOUT><BROWSE comp="TreeView" w="396"/></BROWSELAYOUT>
  <VALUE name="TableHeader-PlaylistTracks"><TABLELAYOUT>
    <COLUMN id="21" visible="1" width="350"/>
    <COLUMN id="29" visible="1" width="80"/>
  </TABLELAYOUT></VALUE>
</PROPERTIES>`;

describe("startup browse sync", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.mocked(getBackend).mockReset();
  });

  it("imports on startup by default", async () => {
    vi.mocked(getBackend).mockResolvedValue({ rekordboxBrowseSettings: () => Promise.resolve(xml) } as Awaited<ReturnType<typeof getBackend>>);
    await syncRekordboxBrowseAtStartup();
    expect(JSON.parse(localStorage.getItem("rbl.columns.v2.playlist") ?? "null").order).toEqual(["title", "bpm"]);
    expect(JSON.parse(localStorage.getItem("rbl.session") ?? "null").treeWidth).toBe(396);
  });

  it("does not read rekordbox or replace layouts when turned off", async () => {
    localStorage.setItem("rbl.preferences", JSON.stringify({ rekordbox: { syncBrowseSettings: false } }));
    localStorage.setItem("rbl.columns.v2.playlist", "saved");
    await syncRekordboxBrowseAtStartup();
    expect(getBackend).not.toHaveBeenCalled();
    expect(localStorage.getItem("rbl.columns.v2.playlist")).toBe("saved");
  });
});
