import { useEffect } from 'preact/hooks';

/** Order of the sidebar tabs (clipboard → toolbox). */
const TABS = ['/', '/toolbox'];

/** Current hash path without leading '#' or query string. */
function basePath(): string {
  const hash = window.location.hash;
  const p = hash.startsWith('#') ? hash.slice(1) : hash;
  const quest = p.indexOf('?');
  return quest >= 0 ? p.slice(0, quest) : p;
}

/**
 * Registers `[` and `]` to switch between the sidebar tabs (剪贴板 / 工具箱).
 * `]` advances to the next tab, `[` goes back. Matches existing shortcut style
 * (document-level listener, no input guard) so it works even while search is focused.
 */
export function useSidebarTabShortcuts() {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key !== '[' && e.key !== ']') return;
      e.preventDefault();
      const idx = Math.max(0, TABS.indexOf(basePath()));
      const next = e.key === '['
        ? Math.max(0, idx - 1)
        : Math.min(TABS.length - 1, idx + 1);
      window.location.hash = TABS[next];
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, []);
}
