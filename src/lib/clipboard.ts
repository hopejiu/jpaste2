/**
 * Unified clipboard copy utility.
 * All frontend code should use this instead of calling navigator.clipboard directly.
 *
 * Returns `true` on success, `false` on failure (so callers can react instead of
 * silently swallowing the error).
 */

import { error } from './logger';

/** Copy text to clipboard. Returns success status. */
export async function copyToClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch (e) {
    error('clipboard: writeText FAILED', e);
    return false;
  }
}
