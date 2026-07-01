/// <reference types="vitest/globals" />
import { describe, it, expect, beforeEach } from 'vitest';
import { entries, hasMore, loading, refreshEntries } from './use-entries';

// Mock Tauri invoke
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue({
    entries: [],
    has_more: false,
  }),
}));

describe('use-entries signals', () => {
  beforeEach(() => {
    entries.value = [];
    hasMore.value = true;
    loading.value = false;
  });

  it('starts with empty state', () => {
    expect(entries.value).toEqual([]);
    expect(hasMore.value).toBe(true);
    expect(loading.value).toBe(false);
  });

  it('refreshEntries loads entries', async () => {
    await refreshEntries();
    expect(Array.isArray(entries.value)).toBe(true);
  });

  it('does not reload while loading', async () => {
    loading.value = true;
    const prev = entries.value.length;
    await refreshEntries();
    expect(entries.value.length).toBe(prev);
  });
});
