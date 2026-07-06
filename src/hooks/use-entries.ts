import { signal } from '@preact/signals';
import { api } from '../lib/invoke';
import type { Entry } from '../lib/types';
import { debug, error as logError } from '../lib/logger';

// ── Signals ───────────────────────────────────────────────────────────

export const entries = signal<Entry[]>([]);
export const hasMore = signal(true);
export const loading = signal(false);
export const searchQuery = signal('');
export const tagFilter = signal(0);
export const isRegex = signal(false);

// Pagination cursor as signals (was module-level mutable state)
const cursorUpdated = signal(0);
const cursorId = signal(0);

// ponytail: signals are the store; writes are centralized here so components
// read signals directly but never mutate `.value` outside this module.
export function setSearchQuery(v: string) { searchQuery.value = v; }
export function setTagFilter(mask: number) { tagFilter.value = mask; }
export function setIsRegex(v: boolean) { isRegex.value = v; }

// ── Actions ───────────────────────────────────────────────────────────

export async function refreshEntries() {
  if (loading.value) return;
  loading.value = true;
  debug('refreshEntries', { search: searchQuery.value, tagFilter: tagFilter.value });

  try {
    cursorUpdated.value = 0;
    cursorId.value = 0;

    if (isRegex.value && searchQuery.value) {
      const result = await api.getEntriesRegex(searchQuery.value, tagFilter.value);
      entries.value = result;
      hasMore.value = false;
    } else {
      const res = await api.getEntries({
        search: searchQuery.value,
        tagMask: tagFilter.value,
        limit: 20,
      });
      entries.value = res.entries;
      hasMore.value = res.has_more;
      debug('refreshEntries loaded', { count: res.entries.length, hasMore: res.has_more });
      if (res.entries.length > 0) {
        const last = res.entries[res.entries.length - 1];
        cursorUpdated.value = last.updated_at;
        cursorId.value = last.id;
      }
    }
  } catch (e) {
    logError('refreshEntries failed', e);
  } finally {
    loading.value = false;
  }
}

export async function loadMore() {
  if (loading.value || !hasMore.value || isRegex.value) return;
  loading.value = true;
  debug('loadMore', { cursorUpdated: cursorUpdated.value, cursorId: cursorId.value });

  try {
    const result = await api.getEntries({
      search: searchQuery.value,
      tagMask: tagFilter.value,
      cursorUpdated: cursorUpdated.value,
      cursorId: cursorId.value,
      limit: 20,
    });

    entries.value = [...entries.value, ...result.entries];
    hasMore.value = result.has_more;
    debug('loadMore loaded', { newCount: result.entries.length, total: entries.value.length });

    if (result.entries.length > 0) {
      const last = result.entries[result.entries.length - 1];
      cursorUpdated.value = last.updated_at;
      cursorId.value = last.id;
    }
  } catch (e) {
    logError('loadMore failed', e);
  } finally {
    loading.value = false;
  }
}

export async function deleteEntry(id: number) {
  debug('deleteEntry', { id });
  try {
    await api.deleteEntry(id);
    entries.value = entries.value.filter((e) => e.id !== id);
  } catch (e) {
    logError('deleteEntry failed', e);
  }
}

export async function toggleFavorite(id: number, value: boolean) {
  debug('toggleFavorite', { id, value });
  try {
    await api.toggleFavorite(id, value);
    entries.value = entries.value.map((e) =>
      e.id === id ? { ...e, is_favorite: value } : e,
    );
  } catch (e) {
    logError('toggleFavorite failed', e);
  }
}
