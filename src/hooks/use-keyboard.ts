import { useEffect, useRef } from 'preact/hooks';

export interface KeyboardShortcut {
  key: string;
  ctrl?: boolean;
  alt?: boolean;
  shift?: boolean;
  handler: () => void;
}

/**
 * Register keyboard shortcuts for the main page.
 * Uses a ref to always access the latest shortcuts, avoiding stale closure issues.
 */
export function useKeyboardNavigation(shortcuts: KeyboardShortcut[]) {
  // FE-1: Use ref to always have access to latest shortcuts
  const shortcutsRef = useRef(shortcuts);
  useEffect(() => {
    shortcutsRef.current = shortcuts;
  }, [shortcuts]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      for (const s of shortcutsRef.current) {
        const ctrl = s.ctrl ?? false;
        const alt = s.alt ?? false;
        const shift = s.shift ?? false;
        if (
          e.key.toLowerCase() === s.key.toLowerCase() &&
          e.ctrlKey === ctrl &&
          e.altKey === alt &&
          e.shiftKey === shift
        ) {
          e.preventDefault();
          e.stopPropagation();
          s.handler();
          return;
        }
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, []); // Empty deps - listener registered once, uses ref for latest shortcuts
}
