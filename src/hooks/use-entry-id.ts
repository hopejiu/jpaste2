import { useMemo } from 'preact/hooks';

/**
 * Extract entry ID from the current URL hash.
 *
 * All viewer pages (json, image, curl, ws) use the same pattern:
 *   const params = new URLSearchParams(window.location.hash.split('?')[1] ?? '');
 *   const entryId = parseInt(params.get('id') ?? '0');
 *
 * This hook centralizes that logic.
 *
 * Usage:
 *   const entryId = useEntryId(); // returns number, defaults to 0
 */
export function useEntryId(): number {
  return useMemo(() => {
    const hash = window.location.hash;
    const params = new URLSearchParams(hash.split('?')[1] ?? '');
    return parseInt(params.get('id') ?? '0');
  }, []);
}
