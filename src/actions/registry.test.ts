import { describe, it, expect } from 'vitest';
import { register, get, detect, getModules } from './registry';
import type { ActionModule } from './registry';

/**
 * Tests use unique ID prefixes per group to avoid interference from
 * shared ESM module state. The detect function returns up to 3 results
 * sorted by priority, so test assertions account for cumulative state.
 */

function reg(id: string, match: boolean, priority = 50): ActionModule {
  const mod: ActionModule = { id, label: id, priority, detect: () => match };
  register(mod);
  return mod;
}

describe('registry basics', () => {
  it('registers and retrieves', () => {
    reg('b-get1', true);
    expect(get('b-get1')).toBeDefined();
    expect(get('b-get1')!.id).toBe('b-get1');
  });

  it('returns undefined for unknown ID', () => {
    expect(get('b-nonexistent')).toBeUndefined();
  });

  it('getModules returns all registered modules', () => {
    const all = getModules();
    expect(all.some(m => m.id === 'b-get1')).toBe(true);
  });
});

describe('detection behavior', () => {
  // Use a unique high-priority group to dominate the top 3 slots
  it('filters by detector return value', () => {
    reg('d-match', true, 999);
    reg('d-nomatch', false, 998);
    const result = detect('x');
    // d-match should be present (highest priority match)
    expect(result.some(m => m.id === 'd-match')).toBe(true);
    // d-nomatch should NOT be present (detector returns false)
    expect(result.every(m => m.id !== 'd-nomatch')).toBe(true);
  });

  it('sorts by priority descending', () => {
    reg('d-high', true, 9000);
    reg('d-mid', true, 8000);
    reg('d-low', true, 7000);
    const ids = detect('x').map(m => m.id);
    // All three should be in results in order
    expect(ids[0]).toBe('d-high');
    expect(ids[1]).toBe('d-mid');
    expect(ids[2]).toBe('d-low');
  });

  it('caps results at 3', () => {
    reg('d-cap1', true, 600);
    reg('d-cap2', true, 500);
    reg('d-cap3', true, 400);
    reg('d-cap4', true, 300);
    reg('d-cap5', true, 200);
    expect(detect('x').length).toBeLessThanOrEqual(3);
  });
});
