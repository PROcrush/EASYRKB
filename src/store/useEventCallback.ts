import { useCallback, useLayoutEffect, useRef } from "react";

/** Stable event identity, with the latest committed state. Only call from
 * events/subscriptions, never during render. Abandoned renders cannot change
 * the handler seen by an already mounted child or native event listener.
 */
export function useEventCallback<A extends unknown[], R>(callback: (...args: A) => R): (...args: A) => R {
  const current = useRef(callback);
  useLayoutEffect(() => { current.current = callback; });
  return useCallback((...args: A) => current.current(...args), []);
}
