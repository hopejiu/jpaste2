import { useEffect, useState, useCallback, useRef } from 'preact/hooks';
import { api } from '../../lib/invoke';
import { copyToClipboard } from '../../lib/clipboard';
import { error as logError } from '../../lib/logger';
import { WINDOW_SHOWN, WINDOW_HIDING } from '../../lib/events';
import type { Entry } from '../../lib/types';
import {
  entries, hasMore, loading,
  searchQuery, tagFilter,
  setSearchQuery, setTagFilter, setIsRegex,
  refreshEntries, loadMore, deleteEntry, toggleFavorite,
  sortFieldSignal, sortOrderSignal,
} from '../../hooks/use-entries';
import { useKeyboardNavigation } from '../../hooks/use-keyboard';
import { useTauriEvent } from '../../hooks/use-events';
import { useMainShortcuts } from '../../hooks/use-main-shortcuts';
import { useFiloStatus } from '../../hooks/use-filo-status';

/** Center the main window on the current monitor */
async function centerWindow() {
  try {
    const { getCurrentWindow, PhysicalPosition, currentMonitor } = await import('@tauri-apps/api/window');
    const w = getCurrentWindow();
    const monitor = await currentMonitor();
    if (!monitor) return;
    const { width: mw, height: mh } = monitor.size;
    const { x: mx, y: my } = monitor.position;
    const { width: ww, height: wh } = await w.outerSize();
    const x = Math.round(mx + (mw - ww) / 2);
    const y = Math.round(my + (mh - wh) / 2);
    await w.setPosition(new PhysicalPosition(x, y));
  } catch (e) {
    logError('centerWindow', e);
  }
}

/**
 * Owns all MainPage state, effects and event handlers.
 * The view component (`index.tsx`) stays a thin render layer.
 */
export function useMainPage() {
  const [focusedIndex, setFocusedIndex] = useState(-1);
  const [showHelp, setShowHelp] = useState(false);
  const [errorAlert, setErrorAlert] = useState<{ title: string; message: string } | null>(null);
  const [pinned, setPinned] = useState(false);
  const [qrModal, setQrModal] = useState<Entry | null>(null);
  const [qrText, setQrText] = useState('');
  const [qrLoading, setQrLoading] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const hiddenTimeRef = useRef<number | null>(null);

  const { mode: filoMode, items: queueItems, setMode: setFiloMode, refreshItems: refreshFiloItems } = useFiloStatus();

  // Load entries on mount
  useEffect(() => { refreshEntries(); }, []);

  // Load sort settings from saved settings on mount
  useEffect(() => {
    api.getSettings().then((s) => {
      sortFieldSignal.value = s.sort_field || 'updated_at';
      sortOrderSignal.value = s.sort_order || 'desc';
    }).catch((e) => logError('load sort settings', e));
  }, []);

  // Sync pinned state from backend on mount (survives re-navigation)
  useEffect(() => {
    api.getPinned().then(setPinned).catch((e) => logError('get pinned', e));
  }, []);

  // Focus search input and scroll list to top on mount
  // (handles the case where MainPage mounts due to WINDOW_SHOWN navigation from settings)
  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
    const listEl = document.querySelector('.entry-list') as HTMLElement;
    if (listEl) listEl.scrollTop = 0;
  }, []);

  // Debounced reload on search/tag change
  useEffect(() => {
    const timer = setTimeout(() => {
      setFocusedIndex(-1);
      refreshEntries();
    }, 300);
    return () => clearTimeout(timer);
  }, [searchQuery.value, tagFilter.value]);

  // Clipboard update listener
  useTauriEvent('clipboard-updated', () => { refreshEntries(); });

  // Track when window was hidden (for auto-clear + centering)
  useTauriEvent(WINDOW_HIDING, () => {
    hiddenTimeRef.current = Date.now();
  });

  // Window shown — focus search, handle auto-clear, optional centering
  useTauriEvent(WINDOW_SHOWN, async () => {
    refreshEntries();
    setFocusedIndex(-1);
    const now = Date.now();
    const hiddenTime = hiddenTimeRef.current;
    try {
      const settings = await api.getSettings();
      // Auto-clear search only when window was previously hidden
      if (hiddenTime !== null && settings.auto_clear_search) {
        const thresholdMs = (settings.auto_clear_seconds || 30) * 1000;
        if ((now - hiddenTime) >= thresholdMs) {
          setSearchQuery('');
          setTagFilter(0);
          setIsRegex(false);
        }
      }
      // Center window only when returning from hidden state (not on every focus)
      if (hiddenTime !== null && settings.center_on_show) {
        await centerWindow();
      }
    } catch (e) { logError('WINDOW_SHOWN handler', e); }
    hiddenTimeRef.current = null;
    inputRef.current?.focus();
    inputRef.current?.select();
    // Scroll list to top
    const listEl = document.querySelector('.entry-list') as HTMLElement;
    if (listEl) listEl.scrollTop = 0;
  });

  // Clamp focus index
  const currentEntries = entries.value;
  // FP-3: Use ref to store latest entries to avoid callback recreation
  const entriesRef = useRef(currentEntries);
  entriesRef.current = currentEntries;
  useEffect(() => {
    if (focusedIndex >= currentEntries.length && currentEntries.length > 0) {
      setFocusedIndex(currentEntries.length - 1);
    }
  }, [currentEntries.length, focusedIndex]);

  // ── Handlers ──────────────────────────────────────────────────────

  // FE-5: handleSelect and handleCopy are identical — merged into one
  const handleSelect = useCallback(async (entry: Entry) => {
    try {
      // Image entries have empty text content — only the thumbnail opens the
      // viewer (entry-item.tsx). The body / Enter copy or paste the actual
      // image depending on the default action.
      if (entry.has_image) {
        const settings = await api.getSettings();
        if (settings.default_action === 'paste') {
          await api.pasteEntryAndHide(entry.id);
        } else {
          await api.copyEntry(entry.id);
          api.incrementCopyCount(entry.id).catch((e) => logError('incrementCopyCount', e));
          if (settings.auto_hide_after_copy) {
            await api.hideMainWindow();
          }
        }
        return;
      }
      const settings = await api.getSettings();
      if (settings.default_action === 'paste') {
        await api.pasteEntryAndHide(entry.id);
      } else {
        const ok = await copyToClipboard(entry.content);
        if (ok) {
          // Increment copy count for user copy
          api.incrementCopyCount(entry.id).catch((e) => logError('incrementCopyCount', e));
          if (settings.auto_hide_after_copy) {
            await api.hideMainWindow();
          }
        } else {
          // C6: surface the previously-silent copy failure instead of
          // hiding the window with nothing copied.
          setErrorAlert({ title: '复制失败', message: '无法写入剪贴板，请检查剪贴板权限后重试。' });
        }
      }
    } catch (e) { logError('handleSelect', e); }
  }, []);

  const handleImageClick = useCallback((entry: Entry) => {
    api.openViewer('/viewer/image', entry.id);
  }, []);

  const handleQrClick = useCallback(async (entry: Entry) => {
    setQrModal(entry);
    setQrLoading(true);
    setQrText('');
    try {
      const text = await api.scanQrText(entry.id);
      setQrText(text || '未找到二维码内容');
    } catch (e) {
      logError('scanQrText', e);
      setQrText('二维码解析失败');
    }
    setQrLoading(false);
  }, []);

  const handleCopyQr = useCallback((text: string) => {
    if (text && text !== '未找到二维码内容' && text !== '二维码解析失败') {
      copyToClipboard(text);
      setQrModal(null);
    }
  }, []);

  const handleActionClick = useCallback((actionId: string, entry: Entry) => {
    import('../../actions').then(({ get }) => {
      const m = get(actionId);
      if (m?.handler) m.handler(entry.content, entry.id);
    });
  }, []);

  const handleOpenEditor = useCallback((entry: Entry) => {
    api.invoke('open_in_editor', { id: entry.id });
  }, []);

  const handleOpenEditorById = useCallback((id: number) => {
    api.invoke('open_in_editor', { id });
  }, []);

  const handleMoveTo = useCallback((idx: number) => {
    const e = entriesRef.current[idx];
    if (e) handleSelect(e);
  }, [handleSelect]);

  const handleMoveDown = useCallback(() => {
    setFocusedIndex((i) => Math.min(i + 1, entriesRef.current.length - 1));
  }, []);

  const handleMoveUp = useCallback(() => {
    setFocusedIndex((i) => Math.max(i - 1, 0));
  }, []);

  const handleDelete = useCallback(async () => {
    const e = entriesRef.current[focusedIndex];
    if (e) {
      await deleteEntry(e.id);
      setFocusedIndex((i) => Math.min(i, entriesRef.current.length - 2));
    }
  }, [focusedIndex]);

  const handleToggleFav = useCallback(async () => {
    const e = entriesRef.current[focusedIndex];
    if (e) await toggleFavorite(e.id, !e.is_favorite);
  }, [focusedIndex]);

  const handlePageUp = useCallback(() => {
    const listEl = document.querySelector('.entry-list') as HTMLElement;
    if (listEl) listEl.scrollTop -= listEl.clientHeight * 0.8;
  }, []);

  const handlePageDown = useCallback(() => {
    const listEl = document.querySelector('.entry-list') as HTMLElement;
    if (listEl) listEl.scrollTop += listEl.clientHeight * 0.8;
  }, []);

  const handleCycleTab = useCallback((direction: 1 | -1) => {
    const tabs = [0, 1, 4, 8, 16, 32];
    const currentIdx = tabs.indexOf(tagFilter.value);
    const nextIdx = (currentIdx + direction + tabs.length) % tabs.length;
    setTagFilter(tabs[nextIdx]);
    setFocusedIndex(-1);
  }, [tagFilter.value]);

  const handleEscape = useCallback(() => {
    if (searchQuery.value) {
      setSearchQuery('');
    } else {
      api.invoke('hide_main_window');
    }
  }, []);

  const handleFocusSearch = useCallback(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  const handleSortChange = useCallback((field: string, order: string) => {
    sortFieldSignal.value = field;
    sortOrderSignal.value = order;
    api.getSettings().then((s) => {
      api.saveSettings({ ...s, sort_field: field, sort_order: order });
    }).catch((e) => logError('save sort settings', e));
    setTimeout(() => refreshEntries(), 0);
  }, []);

  // Keyboard shortcuts
  const shortcuts = useMainShortcuts({
    entries: currentEntries,
    focusedIndex,
    onMoveDown: handleMoveDown,
    onMoveUp: handleMoveUp,
    onSelect: handleSelect,
    onDelete: handleDelete,
    onToggleFav: handleToggleFav,
    onMoveTo: handleMoveTo,
    onPageUp: handlePageUp,
    onPageDown: handlePageDown,
    onCycleTab: handleCycleTab,
    onEscape: handleEscape,
    onCopy: handleSelect,
    onOpenEditor: handleOpenEditor,
    onFocusSearch: handleFocusSearch,
  });
  useKeyboardNavigation(shortcuts);

  // F12 → open DevTools
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'F12') {
        api.invoke('open_devtools');
      }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, []);

  const setPasteOrder = useCallback(async (mode: string) => {
    await setFiloMode(mode);
  }, [setFiloMode]);

  return {
    inputRef,
    focusedIndex, setFocusedIndex,
    showHelp, setShowHelp,
    errorAlert, setErrorAlert,
    pinned, setPinned,
    qrModal, qrText, qrLoading, setQrModal,
    filoMode, queueItems, setFiloMode, refreshFiloItems,
    sortField: sortFieldSignal.value,
    sortOrder: sortOrderSignal.value,
    currentEntries,
    hasMoreVal: hasMore.value,
    loadingVal: loading.value,
    loadMore,
    handleSelect, handleDelete, handleToggleFav,
    handleImageClick, handleQrClick, handleCopyQr,
    handleActionClick, handleOpenEditorById,
    handleSortChange, setPasteOrder,
    handleEscape, handleFocusSearch,
  };
}
