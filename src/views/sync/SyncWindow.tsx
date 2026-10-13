/**
 * The Sync Manager as a window of its own.
 *
 * The shell opens a second webview at `index.html#sync`, and this is what
 * it renders: the same manager the browser build draws over itself, filling
 * a window the platform gives a title bar to drag by. The preferences it
 * reads — the DJ System defaults a fresh stick is given — come from the
 * storage both windows share.
 */
import { useCallback, useState } from "react";

import { getBackend } from "@/ipc/client";
import { useShowWindowWhenReady } from "@/lib/windowReady";
import { PreferencesProvider, usePreferencesStore } from "@/store/usePreferences";
import { SyncManager } from "./SyncManager";

export function SyncWindow() {
  const [ready, setReady] = useState(false);
  useShowWindowWhenReady(ready);
  const markReady = useCallback(() => setReady(true), []);
  const store = usePreferencesStore();
  const close = useCallback(() => {
    void getBackend().then((backend) => backend.closeWindow());
  }, []);
  return (
    <PreferencesProvider value={store}>
      <SyncManager windowed onClose={close} onReady={markReady} />
    </PreferencesProvider>
  );
}
