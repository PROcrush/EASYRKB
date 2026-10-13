/**
 * The Explorer's state: where it starts, and which folders have been opened.
 *
 * Roots are asked for once. A folder's children are asked for the first time
 * it is opened and kept — closing and reopening costs nothing, and the disk
 * is read exactly as far as somebody has clicked. The nodes come out through
 * `explorerNodes`, so the tree draws them like any other section.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { getBackend } from "@/ipc/client";
import type { ExplorerChildren, ExplorerRoot, TreeNode } from "@/ipc/types";
import { explorerNodes, explorerPath } from "@/lib/explorer";

export interface Explorer {
  nodes: TreeNode[];
  /** Reads a folder's subfolders, once; later calls for the same folder are free. */
  expand: (node: TreeNode) => void;
}

export function useExplorer(): Explorer {
  const [roots, setRoots] = useState<readonly ExplorerRoot[]>([]);
  // A new map on every change rather than a mutated one, so the memo below
  // sees it. A folder answered with nothing is in here too — an empty array —
  // which is what stops an unreadable folder being asked for on every open.
  const [children, setChildren] = useState<ReadonlyMap<string, ExplorerChildren>>(
    () => new Map(),
  );
  // Folders asked for and not yet answered, so a double-click on a twisty
  // does not read the same directory twice.
  const pending = useRef<Set<string>>(new Set());

  useEffect(() => {
    let live = true;
    void (async () => {
      const backend = await getBackend();
      try {
        const found = await backend.explorerRoots();
        if (live) setRoots(found);
      } catch {
        // No roots is an Explorer with nothing in it, which the rail dims;
        // a backend that cannot list volumes is not a reason to lose the
        // library.
      }
    })();
    return () => {
      live = false;
    };
  }, []);

  const expand = useCallback(
    (node: TreeNode) => {
      const path = explorerPath(node.id);
      if (path === null || children.has(path) || pending.current.has(path)) return;
      pending.current.add(path);
      void (async () => {
        const backend = await getBackend();
        let found: ExplorerChildren = { names: [], total: 0 };
        try {
          found = await backend.explorerChildren(path);
        } catch {
          // Unreadable, or gone: an empty branch, as the backend itself
          // answers for a folder it may not open.
        }
        pending.current.delete(path);
        setChildren((known) => new Map(known).set(path, found));
      })();
    },
    [children],
  );

  const nodes = useMemo(() => explorerNodes(roots, children), [roots, children]);

  return useMemo(() => ({ nodes, expand }), [nodes, expand]);
}
