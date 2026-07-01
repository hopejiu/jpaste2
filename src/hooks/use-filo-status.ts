import { useEffect, useCallback } from 'preact/hooks';
import { signal } from '@preact/signals';
import { listen } from '@tauri-apps/api/event';
import { api } from '../lib/invoke';
import { PASTE_ORDER_CHANGED } from '../lib/events';

export interface FiloStatus {
  mode: string;
  items: string[];
}

// FE-3: Use signals for consistent state management
const filoMode = signal('normal');
const filoItems = signal<string[]>([]);

/**
 * Manage filo (paste queue) status.
 * Fetches initial status and provides mode switching.
 */
export function useFiloStatus() {
  const fetchStatus = useCallback(async () => {
    try {
      const s = await api.getFiloStatus();
      filoMode.value = s.mode;
      filoItems.value = s.items ?? [];
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    fetchStatus();
  }, [fetchStatus]);

  const setModeAndUpdate = useCallback(async (newMode: string) => {
    await api.filoSetMode(newMode);
    filoMode.value = newMode;
    if (newMode === 'normal') {
      filoItems.value = [];
    }
  }, []);

  const refreshItems = useCallback(async () => {
    try {
      const s = await api.getFiloStatus();
      filoItems.value = s.items ?? [];
    } catch {
      /* ignore */
    }
  }, []);

  // Backend can switch mode on its own (queue auto-reset on empty/clear).
  // Keep the UI in sync when that happens.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<string>(PASTE_ORDER_CHANGED, (e) => {
      filoMode.value = e.payload;
      if (e.payload === 'normal') {
        filoItems.value = [];
      } else {
        refreshItems();
      }
    }).then((u) => { unlisten = u; });
    return () => { unlisten?.(); };
  }, [refreshItems]);

  return { mode: filoMode, items: filoItems, setMode: setModeAndUpdate, refreshItems };
}
