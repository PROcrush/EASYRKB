import { createContext, useContext, useLayoutEffect, useMemo, useState, useSyncExternalStore, type ReactNode } from "react";
import { emptyVu } from "@/lib/vuMeter";
import type { VuMeterMode } from "@/lib/preferences";
import { useMaster, type Master } from "./useMaster";

function createStore() {
  let current: Master = { level: 1, peakLeft: 0, peakRight: 0, reduction: 0, vu: emptyVu("normal"), setLevel: () => {} };
  const listeners = new Set<() => void>();
  return {
    getSnapshot: () => current,
    getLevel: () => current.level,
    setLevel: (level: number) => current.setLevel(level),
    subscribe: (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener); }; },
    publish: (next: Master) => {
      if (current === next) return;
      current = next;
      for (const listener of listeners) listener();
    },
  };
}
const Context = createContext<ReturnType<typeof createStore> | null>(null);
function useStore() {
  const store = useContext(Context);
  if (!store) throw new Error("MasterOutputProvider is missing");
  return store;
}

export function MasterOutputProvider({ children }: { children: ReactNode }) {
  const [store] = useState(createStore);
  return <Context.Provider value={store}>{children}</Context.Provider>;
}

/** One engine subscription and fall animation per window, outside the shell. */
export function MasterOutputConnection({ mode }: { mode: VuMeterMode }) {
  const store = useStore();
  const master = useMaster(mode);
  useLayoutEffect(() => { store.publish(master); }, [store, master]);
  return null;
}

export function useMasterDisplay(): Master {
  const store = useStore();
  return useSyncExternalStore(store.subscribe, store.getSnapshot);
}

/** Meter-only changes do not notify React's level consumer. */
export function useMasterControls() {
  const store = useStore();
  const level = useSyncExternalStore(store.subscribe, store.getLevel);
  return useMemo(() => ({ level, setLevel: store.setLevel }), [level, store]);
}
