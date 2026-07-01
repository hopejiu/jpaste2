import { useState, useEffect } from 'preact/hooks';

/**
 * Extract entry ID from the current URL hash.
 *
 * All viewer pages (json, image, curl, ws) use the same pattern:
 *   const params = new URLSearchParams(window.location.hash.split('?')[1] ?? '');
 *   const entryId = parseInt(params.get('id') ?? '0');
 *
 * This hook centralizes that logic and stays reactive to hash changes
 * (Rust sets `window.location.hash` after the window loads, so a plain
 * useMemo would capture the empty initial hash).
 *
 * Usage:
 *   const entryId = useEntryId(); // returns number, defaults to 0
 */
export function useEntryId(): number {
  const readId = () => {
    const hash = window.location.hash;
    const params = new URLSearchParams(hash.split('?')[1] ?? '');
    return parseInt(params.get('id') ?? '0');
  };

  const [entryId, setEntryId] = useState(readId);

  useEffect(() => {
    // Poll briefly for hash changes — covers the case where Rust sets the
    // hash after the window loads (useMemo would miss this). Stops after
    // 2s since by then the hash is guaranteed to be set.
    let timer = setInterval(() => {
      const next = readId();
      setEntryId((prev) => (prev !== next ? next : prev));
    }, 50);

    const stopTimer = () => { clearInterval(timer); timer = undefined as any; };
    const safety = setTimeout(stopTimer, 2000);

    // Also listen for hashchange event (covers SPA navigation).
    const onHashChange = () => setEntryId(readId());
    window.addEventListener('hashchange', onHashChange);

    return () => {
      clearTimeout(safety);
      if (timer) clearInterval(timer);
      window.removeEventListener('hashchange', onHashChange);
    };
  }, []);

  return entryId;
}
