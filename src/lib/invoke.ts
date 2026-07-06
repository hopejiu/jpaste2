import { invoke } from '@tauri-apps/api/core';
export { invoke } from '@tauri-apps/api/core';
import type {
  Entry, QueryResult, Settings, FiloStatus, Stats, CleanupResult,
} from './types';

export const api = {
  // ── History ──────────────────────────────────────────────────────────
  getEntries(
    params: {
      search?: string;
      tagMask?: number;
      cursorUpdated?: number;
      cursorId?: number;
      limit?: number;
      sortField?: string;
      sortOrder?: string;
    },
  ): Promise<QueryResult> {
    return invoke('get_entries', {
      search: params.search ?? '',
      tagMask: params.tagMask ?? 0,
      cursorUpdated: params.cursorUpdated ?? 0,
      cursorId: params.cursorId ?? 0,
      limit: params.limit ?? 20,
      sortField: params.sortField ?? 'updated_at',
      sortOrder: params.sortOrder ?? 'DESC',
    });
  },

  getEntryContent(id: number): Promise<string> {
    return invoke('get_entry_content', { id });
  },

  getEntryImage(id: number): Promise<string> {
    return invoke('get_entry_image', { id });
  },

  getEntryImageFull(id: number): Promise<string> {
    return invoke('get_entry_image_full', { id });
  },

  deleteEntry(id: number): Promise<boolean> {
    return invoke('delete_entry', { id });
  },

  toggleFavorite(id: number, value: boolean): Promise<void> {
    return invoke('toggle_favorite', { id, value });
  },

  cleanup(retainDays: number): Promise<CleanupResult> {
    return invoke('cleanup', { retainDays });
  },

  clearAll(keepFavorites: boolean): Promise<void> {
    return invoke('clear_all', { keepFavorites });
  },

  getStats(): Promise<Stats> {
    return invoke('get_stats');
  },

  // ── Settings ─────────────────────────────────────────────────────────
  getSettings(): Promise<Settings> {
    return invoke('get_settings');
  },

  saveSettings(data: Settings): Promise<void> {
    return invoke('save_settings', { data });
  },

  // ── FiloStack ────────────────────────────────────────────────────────
  getFiloStatus(): Promise<FiloStatus> {
    return invoke('get_filo_status');
  },

  filoSetMode(mode: string): Promise<void> {
    return invoke('filo_set_mode', { mode });
  },

  filoClear(): Promise<void> {
    return invoke('filo_clear');
  },

  // ── Auto Start ──────────────────────────────────────────────────────
  enableAutostart(): Promise<void> {
    return invoke('enable_autostart');
  },

  disableAutostart(): Promise<void> {
    return invoke('disable_autostart');
  },

  isAutostartEnabled(): Promise<boolean> {
    return invoke('is_autostart_enabled');
  },

  // ── Clipboard ───────────────────────────────────────────────────────
  pasteEntry(id: number): Promise<void> {
    return invoke('paste_entry', { id });
  },

  pasteEntryAndHide(id: number): Promise<void> {
    return invoke('paste_entry_and_hide', { id });
  },

  // ── Window ─────────────────────────────────────────────────────────
  hideMainWindow(): Promise<void> {
    return invoke('hide_main_window');
  },

  // ── Viewer windows ─────────────────────────────────────────────────
  // Single registry-driven command (see src-tauri/src/command/viewer.rs).
  openViewer(route: string, id: number): Promise<void> {
    return invoke('open_viewer', { route, id });
  },

  // ── Open URL ───────────────────────────────────────────────────────
  openUrl(url: string): Promise<void> {
    return invoke('open_url', { url });
  },

  // ── Image Viewer ────────────────────────────────────────────────────
  getEntriesRegex(pattern: string, tagMask?: number): Promise<Entry[]> {
    return invoke('get_entries_regex', {
      pattern,
      tagMask: tagMask ?? 0,
      sortField: 'updated_at',
      sortOrder: 'DESC',
    });
  },

  getImageList(tagMask?: number, search?: string): Promise<number[]> {
    return invoke('get_image_list', { tagMask: tagMask ?? 0, search: search ?? '' });
  },

  // ── Curl ────────────────────────────────────────────────────────────
  sendCurlRequest(params: {
    method: string;
    url: string;
    headers: Record<string, string>;
    body: string;
    followRedirects: boolean;
    timeout: number;
  }): Promise<{
    status_code: number;
    status_text: string;
    headers: Record<string, string>;
    body: string;
    duration_ms: number;
  }> {
    return invoke('send_curl_request', params);
  },

  // ── Pinned ──────────────────────────────────────────────────────────
  togglePinned(): Promise<boolean> {
    return invoke('toggle_pinned');
  },

  getPinned(): Promise<boolean> {
    return invoke('get_pinned');
  },

  // ── Toast ──────────────────────────────────────────────────────────
  showToast(message: string): Promise<void> {
    return invoke('show_toast', { message });
  },

  // ── Copy Count ────────────────────────────────────────────────────
  incrementCopyCount(id: number): Promise<void> {
    return invoke('increment_copy_count', { id });
  },

  // ── QR Code ──────────────────────────────────────────────────────
  scanQrText(id: number): Promise<string> {
    return invoke('scan_qr_text', { id });
  },

  // ── Generic invoke ─────────────────────────────────────────────────
  // Escape hatch for commands without a typed wrapper
  invoke<T = void>(cmd: string, args?: Record<string, unknown>): Promise<T> {
    return invoke(cmd, args);
  },
};
