import { useEffect, useRef } from 'preact/hooks';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

/**
 * Listen to a Tauri event and call handler when triggered.
 * Uses a ref to store the latest handler, avoiding listener re-registration
 * on every render when the handler reference changes.
 * Cleans up on unmount.
 */
export function useTauriEvent<T = unknown>(
  event: string,
  handler: (payload: T) => void,
) {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    const setup = async () => {
      unlisten = await listen<T>(event, (e) => handlerRef.current(e.payload));
    };
    setup();
    return () => {
      unlisten?.();
    };
  }, [event]); // Only re-register when event name changes
}
