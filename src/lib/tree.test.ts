import { describe, expect, it } from "vitest";

import { TREE_ROOT, type TreeNode } from "@/ipc/types";
import {
  RELATED_NODES, TAG_LIST_NODE, branchIds, childrenOf, containerOf, emptySources, hasChildren,
  newlyClosed, nodesForSource, parentFor, relatedCriterionOf, sourceOf, subtreeIds, toggle,
  visibleNodes, withSources, searchTree,
} from "./tree";

/** `"a"` at depth 0, `"  b"` at depth 1, and so on. */
function tree(...spec: string[]): TreeNode[] {
  return spec.map((s) => {
    const name = s.trimStart();
    return {
      id: name,
      name,
      kind: "playlist" as const,
      depth: (s.length - name.length) / 2,
    };
  });
}

const NESTED = tree(
  "all",
  "gigs",
  "  friday",
  "    warmup",
  "    peak",
  "  saturday",
  "archive",
  "  2025",
);

describe("visibleNodes", () => {
  it("returns everything when nothing is collapsed", () => {
    expect(visibleNodes(NESTED, new Set()).map((n) => n.id)).toEqual(
      NESTED.map((n) => n.id),
    );
  });

  it("hides everything under a collapsed node", () => {
    const visible = visibleNodes(NESTED, new Set(["gigs"])).map((n) => n.id);
    expect(visible).toEqual(["all", "gigs", "archive", "2025"]);
  });

  it("hides a subtree without hiding its siblings", () => {
    const visible = visibleNodes(NESTED, new Set(["friday"])).map((n) => n.id);
    expect(visible).toEqual(["all", "gigs", "friday", "saturday", "archive", "2025"]);
  });

  it("keeps the collapsed node itself visible", () => {
    expect(visibleNodes(NESTED, new Set(["gigs"])).map((n) => n.id)).toContain("gigs");
  });

  it("handles several collapsed nodes at once, nested and not", () => {
    const visible = visibleNodes(NESTED, new Set(["friday", "archive"])).map((n) => n.id);
    expect(visible).toEqual(["all", "gigs", "friday", "saturday", "archive"]);
  });

  it("a collapsed node inside a collapsed node changes nothing", () => {
    const outer = visibleNodes(NESTED, new Set(["gigs"])).map((n) => n.id);
    const both = visibleNodes(NESTED, new Set(["gigs", "friday"])).map((n) => n.id);
    expect(both).toEqual(outer);
  });

  it("collapsing a leaf does nothing", () => {
    expect(visibleNodes(NESTED, new Set(["peak"])).map((n) => n.id)).toEqual(
      NESTED.map((n) => n.id),
    );
  });

  it("survives an id that is not in the tree", () => {
    expect(visibleNodes(NESTED, new Set(["nope"]))).toHaveLength(NESTED.length);
  });

  it("copes with an empty tree", () => {
    expect(visibleNodes([], new Set(["a"]))).toEqual([]);
  });

  it("does not lose a node whose depth jumps back by more than one", () => {
    // "peak" is depth 2 and "archive" is depth 0: the hidden-depth marker has
    // to clear on any shallower node, not only the next one down.
    const visible = visibleNodes(NESTED, new Set(["friday"])).map((n) => n.id);
    expect(visible).toContain("archive");
  });
});

describe("hasChildren", () => {
  it("is true only when the next node is deeper", () => {
    expect(hasChildren(NESTED, 1)).toBe(true); // gigs
    expect(hasChildren(NESTED, 2)).toBe(true); // friday
    expect(hasChildren(NESTED, 3)).toBe(false); // warmup
    expect(hasChildren(NESTED, 0)).toBe(false); // all, next is a sibling
  });

  it("is false for the last node, which can have nothing under it", () => {
    expect(hasChildren(NESTED, NESTED.length - 1)).toBe(false);
    expect(hasChildren(NESTED, 99)).toBe(false);
  });
});

describe("toggle", () => {
  it("adds then removes", () => {
    const once = toggle(new Set<string>(), "a");
    expect([...once]).toEqual(["a"]);
    expect([...toggle(once, "a")]).toEqual([]);
  });

  it("returns a new set, so React sees the change", () => {
    const before = new Set(["a"]);
    expect(toggle(before, "b")).not.toBe(before);
    expect([...before]).toEqual(["a"]);
  });
});

describe("branchIds", () => {
  it("names every node with something under it, and nothing else", () => {
    // archive counts: 2025 sits under it.
    expect([...branchIds(NESTED)].sort()).toEqual(["archive", "friday", "gigs"]);
  });

  it("agrees with hasChildren on every node", () => {
    // The fast path and the obvious one must not drift apart.
    const ids = branchIds(NESTED);
    NESTED.forEach((node, i) => {
      expect(ids.has(node.id)).toBe(hasChildren(NESTED, i));
    });
  });

  it("is empty for a flat tree", () => {
    expect(branchIds(tree("a", "b", "c")).size).toBe(0);
  });
});

describe("nodesForSource", () => {
  const mixed: TreeNode[] = [
    { id: "all", name: "All Tracks", kind: "allTracks", depth: 0 },
    { id: "pl", name: "Playlists", kind: "collection", depth: 0 },
    { id: "f", name: "Gigs", kind: "folder", depth: 1 },
    { id: "p", name: "Friday", kind: "playlist", depth: 2 },
    { id: "h", name: "2026-09-07", kind: "history", depth: 1 },
  ];

  it("gives playlists All Tracks, the folders and the lists", () => {
    expect(nodesForSource(mixed, "playlists").map((n) => n.id)).toEqual(["all", "pl", "f", "p"]);
  });

  it("gives histories the history nodes", () => {
    expect(nodesForSource(mixed, "histories").map((n) => n.id)).toEqual(["h"]);
  });

  it("gives devices nothing, because device support is not built", () => {
    expect(nodesForSource(mixed, "devices")).toEqual([]);
  });

  it("never invents a node", () => {
    const every = (["playlists", "histories", "devices"] as const)
      .flatMap((s) => nodesForSource(mixed, s));
    for (const node of every) expect(mixed).toContain(node);
  });
});

describe("emptySources", () => {
  it("names the sections with nothing in them", () => {
    const only = [{ id: "all", name: "All Tracks", kind: "allTracks" as const, depth: 0 }];
    const empty = emptySources(only);
    // All Tracks is in the playlists section, so that one leads somewhere.
    expect(empty.has("playlists")).toBe(false);
    expect(empty.has("histories")).toBe(true);
    expect(empty.has("devices")).toBe(true);
  });

  it("calls everything empty for an empty tree", () => {
    expect(emptySources([]).size).toBe(4);
  });
});

describe("sourceOf", () => {
  const mixed: TreeNode[] = [
    { id: "all", name: "All Tracks", kind: "allTracks", depth: 0 },
    { id: "p", name: "Friday", kind: "playlist", depth: 1 },
    { id: "h", name: "2026-09-07", kind: "history", depth: 1 },
  ];

  it("reports the section the selection is in", () => {
    expect(sourceOf(mixed, "p")).toBe("playlists");
    expect(sourceOf(mixed, "h")).toBe("histories");
  });

  it("falls back to playlists, which is where the tree opens", () => {
    // All Tracks included: it sits at the top of the same tree and the rail
    // has no button of its own for it.
    expect(sourceOf(mixed, "all")).toBe("playlists");
    expect(sourceOf(mixed, null)).toBe("playlists");
    expect(sourceOf(mixed, "gone")).toBe("playlists");
  });
});

describe("devices in the tree", () => {
  const nodes: TreeNode[] = [
    { id: "playlists", name: "Playlists", kind: "collection", depth: 0 },
    { id: "pl-1", name: "Warm Up", kind: "playlist", depth: 1 },
    { id: "device:/Volumes/DJ STICK", name: "DJ STICK", kind: "device", depth: 0 },
  ];

  it("shows connected volumes under Devices and nowhere else", () => {
    expect(nodesForSource(nodes, "devices").map((n) => n.name)).toEqual(["DJ STICK"]);
    expect(nodesForSource(nodes, "playlists").map((n) => n.name)).toEqual(["Playlists", "Warm Up"]);
  });

  it("stops dimming Devices once something is connected", () => {
    expect(emptySources(nodes).has("devices")).toBe(false);
    expect(emptySources(nodes.slice(0, 2)).has("devices")).toBe(true);
  });

  it("puts the rail on Devices when a volume is selected", () => {
    expect(sourceOf(nodes, "device:/Volumes/DJ STICK")).toBe("devices");
  });
});

describe("the Explorer in the tree", () => {
  const nodes: TreeNode[] = [
    { id: "playlists", name: "Playlists", kind: "collection", depth: 0 },
    { id: "pl-1", name: "Warm Up", kind: "playlist", depth: 1 },
    { id: "explorer", name: "Explorer", kind: "explorer", depth: 0, expanded: true },
    { id: "dir:0:/Users/x/Music", name: "Music", kind: "directory", depth: 1, expanded: false, lazy: true },
    { id: "dir:2:/", name: "Macintosh HD", kind: "directory", depth: 1, expanded: false, lazy: true },
  ];

  it("is its own section, heading first", () => {
    expect(nodesForSource(nodes, "explorer").map((n) => n.name)).toEqual([
      "Explorer", "Music", "Macintosh HD",
    ]);
    expect(nodesForSource(nodes, "playlists").map((n) => n.name)).toEqual(["Playlists", "Warm Up"]);
    expect(emptySources(nodes).has("explorer")).toBe(false);
    expect(emptySources(nodes.slice(0, 2)).has("explorer")).toBe(true);
  });

  it("puts the rail on Explorer for the heading and for a folder", () => {
    expect(sourceOf(nodes, "explorer")).toBe("explorer");
    expect(sourceOf(nodes, "dir:2:/")).toBe("explorer");
  });

  it("treats a lazy folder as a branch before anything is under it", () => {
    const ids = branchIds(nodes);
    expect(ids.has("dir:2:/")).toBe(true);
    expect(ids.has("dir:0:/Users/x/Music")).toBe(true);
    // The heading has children in the array, so it is a branch the usual way.
    expect(ids.has("explorer")).toBe(true);
    expect(ids.has("pl-1")).toBe(false);
  });
});

describe("newlyClosed", () => {
  const first: TreeNode[] = [
    { id: "a", name: "a", kind: "folder", depth: 0, expanded: true },
    { id: "b", name: "b", kind: "folder", depth: 0, expanded: false },
    { id: "c", name: "c", kind: "playlist", depth: 1 },
  ];

  it("names the closed nodes the tree has not seen", () => {
    expect(newlyClosed(first, new Set())).toEqual(["b"]);
  });

  it("leaves alone what has been seen, whatever the user did to it since", () => {
    const later = [...first, { id: "d", name: "d", kind: "directory" as const, depth: 1, expanded: false }];
    expect(newlyClosed(later, new Set(["a", "b", "c"]))).toEqual(["d"]);
    expect(newlyClosed(later, new Set(["a", "b", "c", "d"]))).toEqual([]);
  });
});

describe("parentFor", () => {
  const nodes: TreeNode[] = [
    { id: "playlists", name: "Playlists", kind: "collection", depth: 0 },
    { id: "top", name: "Top", kind: "playlist", depth: 1 },
    { id: "gigs", name: "Gigs", kind: "folder", depth: 1, expanded: true },
    { id: "friday", name: "Friday", kind: "playlist", depth: 2 },
    { id: "inner", name: "Inner", kind: "folder", depth: 2, expanded: true },
    { id: "deep", name: "Deep", kind: "playlist", depth: 3 },
    { id: "saturday", name: "Saturday", kind: "playlist", depth: 2 },
  ];
  const by = (id: string) => nodes.find((n) => n.id === id)!;

  it("a folder holds what is made from its menu", () => {
    expect(parentFor(nodes, by("gigs"))).toBe("gigs");
  });

  it("a playlist's own folder does, however far down", () => {
    expect(parentFor(nodes, by("friday"))).toBe("gigs");
    expect(parentFor(nodes, by("deep"))).toBe("inner");
    // Past a deeper sibling branch to the folder it is actually in.
    expect(parentFor(nodes, by("saturday"))).toBe("gigs");
  });

  it("a playlist at the top goes at the top, under the backend's root id", () => {
    // The `Playlists` heading is not a folder; "" is not a parent the
    // writer knows, and was what the tree used to send.
    expect(parentFor(nodes, by("top"))).toBe(TREE_ROOT);
    expect(parentFor(nodes, { id: "gone", name: "Gone", kind: "playlist", depth: 1 })).toBe(TREE_ROOT);
  });
});

/** A tree shaped like the real one: the headings, then folders and lists. */
const nested: TreeNode[] = [
  { id: "all", name: "All Tracks", kind: "allTracks", depth: 0 },
  { id: "pl", name: "Playlists", kind: "collection", depth: 0 },
  { id: "one", name: "One", kind: "playlist", depth: 1 },
  { id: "gigs", name: "Gigs", kind: "folder", depth: 1 },
  { id: "fri", name: "Friday", kind: "playlist", depth: 2 },
  { id: "inner", name: "Inner", kind: "folder", depth: 2 },
  { id: "deep", name: "Deep", kind: "playlist", depth: 3 },
  { id: "sat", name: "Saturday", kind: "playlist", depth: 2 },
  { id: "two", name: "Two", kind: "playlist", depth: 1 },
  { id: "hist", name: "Histories", kind: "histories", depth: 0 },
  { id: "sesh", name: "2026-09-07", kind: "history", depth: 1 },
];

describe("containerOf", () => {
  it("gives the folder a node is in, and the root for the top level", () => {
    expect(containerOf(nested, nested[2]!)).toBe(TREE_ROOT);
    expect(containerOf(nested, nested[3]!)).toBe(TREE_ROOT);
    expect(containerOf(nested, nested[4]!)).toBe("gigs");
    expect(containerOf(nested, nested[6]!)).toBe("inner");
    expect(containerOf(nested, nested[7]!)).toBe("gigs");
  });

  it("is not parentFor, which reads a folder as a destination", () => {
    // The same node: where it is, against where a new playlist would go.
    expect(containerOf(nested, nested[3]!)).toBe(TREE_ROOT);
    expect(parentFor(nested, nested[3]!)).toBe("gigs");
  });
});

describe("childrenOf", () => {
  it("gives the top-level playlists for the root, not the other sources", () => {
    expect(childrenOf(nested, TREE_ROOT).map((n) => n.id)).toEqual(["one", "gigs", "two"]);
  });

  it("gives a folder's own children, not its grandchildren", () => {
    expect(childrenOf(nested, "gigs").map((n) => n.id)).toEqual(["fri", "inner", "sat"]);
    expect(childrenOf(nested, "inner").map((n) => n.id)).toEqual(["deep"]);
  });

  it("is empty for a leaf and for a node that is not there", () => {
    expect(childrenOf(nested, "one")).toEqual([]);
    expect(childrenOf(nested, "nope")).toEqual([]);
  });
});

describe("subtreeIds", () => {
  it("is the node and everything filed under it", () => {
    expect([...subtreeIds(nested, nested[3]!)]).toEqual(["gigs", "fri", "inner", "deep", "sat"]);
    expect([...subtreeIds(nested, nested[5]!)]).toEqual(["inner", "deep"]);
  });

  it("is just the node itself for a leaf", () => {
    expect([...subtreeIds(nested, nested[2]!)]).toEqual(["one"]);
  });
});

describe("withSources", () => {
  const backend: TreeNode[] = [
    { id: "all", name: "All Tracks", kind: "allTracks", depth: 0 },
    { id: "pl", name: "Playlists", kind: "collection", depth: 0 },
    { id: "one", name: "One", kind: "playlist", depth: 1 },
  ];

  it("adds Tag List and hides unfinished Related Tracks", () => {
    const ids = withSources(backend).map((n) => n.id);
    expect(ids).toEqual([
      "all", "pl", "one",
      "tagList",
    ]);
  });

  it("does not stack the rows when the tree is refetched", () => {
    // The tree is rebuilt on every library change, and the frontend's own
    // rows are already in the array it is handed back: without the filter
    // each refetch would add another Related Tracks section.
    expect(withSources(withSources(backend))).toEqual(withSources(backend));
  });

  it("leaves the backend's own nodes alone, in order", () => {
    expect(withSources(backend).slice(0, 3)).toEqual(backend);
    expect(withSources([])).toEqual([TAG_LIST_NODE]);
  });

  it("keeps Related hidden and gives the rail a Tag List", () => {
    const nodes = withSources(backend);
    expect(nodesForSource(nodes, "related").map((n) => n.id)).toEqual([
    ]);
    expect(nodesForSource(nodes, "tagList").map((n) => n.id)).toEqual(["tagList"]);
    // And they stay out of the playlists section, which is the rail's job.
    expect(nodesForSource(nodes, "playlists").map((n) => n.id)).toEqual(["all", "pl", "one"]);
  });

  it("puts the rail on the section a selected row belongs to", () => {
    const nodes = [...withSources(backend), ...RELATED_NODES];
    expect(sourceOf(nodes, "related")).toBe("related");
    expect(sourceOf(nodes, "related:artist")).toBe("related");
    expect(sourceOf(nodes, "tagList")).toBe("tagList");
  });
});

describe("relatedCriterionOf", () => {
  it("names the criterion each row stands for", () => {
    expect(relatedCriterionOf("related:bpmKey")).toBe("bpmKey");
    expect(relatedCriterionOf("related:genreRecent")).toBe("genreRecent");
    expect(relatedCriterionOf("related:artist")).toBe("artist");
    expect(relatedCriterionOf("related:suggestion")).toBe("suggestion");
  });

  it("is null for the heading and for anything else", () => {
    // The heading opens no view of its own; nor does a playlist that happens
    // to be selected when the Related section is showing.
    expect(relatedCriterionOf("related")).toBeNull();
    expect(relatedCriterionOf("tagList")).toBeNull();
    expect(relatedCriterionOf("")).toBeNull();
  });

  it("answers for every criterion row, so none can be drawn without a view", () => {
    for (const node of RELATED_NODES) {
      const criterion = relatedCriterionOf(node.id);
      if (node.kind === "relatedCriterion") expect(criterion).not.toBeNull();
      else expect(criterion).toBeNull();
    }
  });
});

describe("searchTree", () => {
  const nodes: TreeNode[] = [
    { id: "root", name: "Playlists", kind: "collection", depth: 0 },
    { id: "folder", name: "House", kind: "folder", depth: 1 },
    { id: "list", name: "Late House", kind: "playlist", depth: 2 },
    { id: "other", name: "Jazz", kind: "playlist", depth: 1 },
  ];
  it("finds matches inside collapsed branches and keeps their context", () => {
    expect(searchTree(nodes, " late ", "playlist").map(n => n.id)).toEqual(["root", "folder", "list"]);
  });
  it("keeps ordinary and intelligent playlist scopes separate", () => {
    const extended: TreeNode[] = [...nodes, { id: "smart", name: "Late Auto", kind: "smartPlaylist", depth: 1 }];
    expect(searchTree(extended, "late", "playlist").map(n => n.id)).toEqual(["root", "folder", "list"]);
    expect(searchTree(extended, "late", "smartPlaylist").map(n => n.id)).toEqual(["root", "smart"]);
  });
  it("scopes matches to folders and preserves the unfiltered tree for a blank query", () => {
    expect(searchTree(nodes, "house", "folder").map(n => n.id)).toEqual(["root", "folder"]);
    expect(searchTree(nodes, "", "folder")).toEqual(nodes);
    expect(searchTree(nodes, "missing", "all")).toEqual([]);
  });
});
