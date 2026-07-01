import { describe, it, expect } from 'vitest';
import { formatBytes, formatTime, previewContent } from './format';

describe('formatBytes', () => {
  it('formats 0 bytes', () => {
    expect(formatBytes(0)).toBe('0 B');
  });

  it('formats bytes', () => {
    expect(formatBytes(500)).toBe('500 B');
  });

  it('formats KB', () => {
    const r = formatBytes(2048);
    expect(r).toContain('KB');
  });

  it('formats MB', () => {
    const r = formatBytes(2_097_152);
    expect(r).toContain('MB');
  });

  it('handles null/undefined', () => {
    expect(formatBytes(null as any)).toBe('0 B');
    expect(formatBytes(undefined as any)).toBe('0 B');
  });
});

describe('formatTime', () => {
  it('returns empty for empty/undefined', () => {
    expect(formatTime('').rel).toBe('');
    expect(formatTime(null as any).rel).toBe('');
    expect(formatTime(undefined as any).rel).toBe('');
  });

  it('returns "刚刚" for current time (number)', () => {
    const r = formatTime(Date.now());
    expect(r.rel).toBe('刚刚');
  });

  it('returns fallback for invalid date', () => {
    const r = formatTime('not-a-date');
    expect(r.rel).toBe('not-a-date');
  });

  it('returns minutes ago (number)', () => {
    const past = Date.now() - 5 * 60 * 1000;
    expect(formatTime(past).rel).toContain('分钟前');
  });

  it('returns hours ago (number)', () => {
    const past = Date.now() - 2 * 3600 * 1000;
    expect(formatTime(past).rel).toContain('小时前');
  });

  it('returns days ago (number)', () => {
    const past = Date.now() - 3 * 86400 * 1000;
    expect(formatTime(past).rel).toContain('天前');
  });

  it('falls back to ISO string for pre-migration data', () => {
    // Old TEXT timestamps still parse correctly via +Z
    const r = formatTime('2026-01-15 10:30:00.000');
    expect(r.abs).toContain('2026');
  });
});

describe('previewContent', () => {
  it('returns empty for empty string', () => {
    expect(previewContent('')).toBe('');
  });

  it('returns short text unchanged', () => {
    expect(previewContent('hi')).toBe('hi');
  });

  it('truncates long single line', () => {
    const text = 'x'.repeat(400);
    const r = previewContent(text);
    expect(r).toHaveLength(303);
    expect(r).toMatch(/\.\.\.$/);
  });

  it('truncates multi-line to 3 lines', () => {
    const r = previewContent('1\n2\n3\n4\n5');
    expect(r).toContain('...');
    expect(r.split('\n').length).toBeLessThanOrEqual(5);
  });

  it('keeps 3 lines if short', () => {
    expect(previewContent('a\nb\nc')).toBe('a\nb\nc');
  });
});
