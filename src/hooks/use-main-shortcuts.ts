import type { Entry } from '../lib/types';

export interface MainShortcut {
  key: string;
  ctrl?: boolean;
  alt?: boolean;
  shift?: boolean;
  handler: () => void;
}

/**
 * Build keyboard shortcuts for the main page.
 * Returns a stable array of shortcut definitions.
 */
export function useMainShortcuts(opts: {
  entries: Entry[];
  focusedIndex: number;
  onMoveDown: () => void;
  onMoveUp: () => void;
  onSelect: (entry: Entry) => void;
  onDelete: (id: number) => void;
  onToggleFav: (id: number, value: boolean) => void;
  onMoveTo: (idx: number) => void;
  onPageUp: () => void;
  onPageDown: () => void;
  onCycleTab: (direction: 1 | -1) => void;
  onEscape: () => void;
  onCopy: (entry: Entry) => void;
  onOpenEditor: (entry: Entry) => void;
  onFocusSearch: () => void;
}): MainShortcut[] {
  const {
    entries,
    focusedIndex,
    onMoveDown,
    onMoveUp,
    onSelect,
    onDelete,
    onToggleFav,
    onMoveTo,
    onPageUp,
    onPageDown,
    onCycleTab,
    onEscape,
    onCopy,
    onOpenEditor,
    onFocusSearch,
  } = opts;

  // Guard a handler behind the currently focused entry.
  const focused = (fn: (e: Entry) => void) => () => {
    const e = entries[focusedIndex];
    if (e) fn(e);
  };

  const shortcuts: MainShortcut[] = [
    { key: 'ArrowDown', handler: onMoveDown },
    { key: 'ArrowUp', handler: onMoveUp },
    { key: 'Enter', handler: focused(onSelect) },
    { key: 'Delete', handler: focused((e) => onDelete(e.id)) },
    { key: ' ', handler: focused((e) => onToggleFav(e.id, !e.is_favorite)) },
    { key: 'Home', handler: () => onMoveTo(0) },
    { key: 'End', handler: () => onMoveTo(entries.length - 1) },
    { key: 'PageUp', handler: onPageUp },
    { key: 'PageDown', handler: onPageDown },
    { key: 'Tab', handler: () => onCycleTab(1) },
    { shift: true, key: 'Tab', handler: () => onCycleTab(-1) },
    { key: 'Escape', handler: onEscape },
    // Ctrl shortcuts (plain keys + Alt variants are redundant with these)
    { ctrl: true, key: 'l', handler: onFocusSearch },
    { ctrl: true, key: 'e', handler: focused(onOpenEditor) },
    { ctrl: true, key: 'c', handler: focused(onCopy) },
  ];

  // Add Ctrl+1~9 shortcuts
  for (let i = 1; i <= 9; i++) {
    const idx = i - 1;
    shortcuts.push({
      ctrl: true,
      key: String(i),
      handler: () => onMoveTo(idx),
    });
  }

  return shortcuts;
}

/// Bottom-bar hint text derived from the shortcuts above.
/// Lives here alongside the shortcut definitions so that changing a shortcut
/// makes it obvious to update the hint.  Used by `MainPage`'s bottom bar.
export const BOTTOM_HINTS = 'Ctrl+L搜索 · E编辑 · C复制 · Del删除 · Space收藏 · [ ]切换 · Esc隐藏';
