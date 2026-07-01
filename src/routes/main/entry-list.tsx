import { useEffect, useRef, useCallback } from 'preact/hooks';
import type { Entry } from '../../lib/types';
import { EntryItem } from './entry-item';

interface EntryListProps {
  entries: Entry[];
  hasMore: boolean;
  loading: boolean;
  focusedIndex: number;
  onLoadMore: () => void;
  onFocus: (idx: number) => void;
  onSelect: (entry: Entry) => void;
  onDelete: (id: number) => void;
  onToggleFav: (id: number, value: boolean) => void;
  onImageClick: (entry: Entry) => void;
  onActionClick: (actionId: string, entry: Entry) => void;
  onOpenEditor: (id: number) => void;
  onQrClick: (entry: Entry) => void;
}

export function EntryList({
  entries, hasMore, loading, focusedIndex,
  onLoadMore, onFocus, onSelect,
  onDelete, onToggleFav,
  onImageClick,
  onActionClick,
  onOpenEditor,
  onQrClick,
}: EntryListProps) {
  const listRef = useRef<HTMLDivElement>(null);

  const handleScroll = useCallback(() => {
    const el = listRef.current;
    if (!el) return;
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 120) onLoadMore();
  }, [onLoadMore]);

  useEffect(() => {
    const el = listRef.current;
    if (!el) return;
    el.addEventListener('scroll', handleScroll, { passive: true });
    return () => el.removeEventListener('scroll', handleScroll);
  }, [handleScroll]);

  useEffect(() => {
    if (focusedIndex >= 0 && listRef.current) {
      const item = listRef.current.querySelector(`[data-idx="${focusedIndex}"]`) as HTMLElement | null;
      item?.scrollIntoView({ block: 'nearest' });
    }
  }, [focusedIndex]);

  if (entries.length === 0 && !loading) {
    return (
      <div class="entry-list" ref={listRef}>
        <div class="empty-state">
          <p class="empty-title">暂无剪贴板历史</p>
          <p class="empty-sub">复制文本即可开始。jPaste 在后台监听剪贴板。</p>
        </div>
      </div>
    );
  }

  return (
    <div class="entry-list" ref={listRef}>
      {entries.map((entry, idx) => (
        <div key={entry.id} data-idx={idx}>
          <EntryItem
            entry={entry}
            idx={idx}
            focused={idx === focusedIndex}
            onActivate={() => onSelect(entry)}
            onDelete={() => onDelete(entry.id)}
            onToggleFav={() => onToggleFav(entry.id, !entry.is_favorite)}
            onOpenEditor={() => onOpenEditor(entry.id)}
            onActionClick={(actionId: string) => onActionClick(actionId, entry)}
            onImageClick={() => onImageClick(entry)}
            onQrClick={() => onQrClick(entry)}
            onFocus={() => onFocus(idx)}
          />
        </div>
      ))}
      {loading && <div class="list-loading">加载中...</div>}
      {hasMore && !loading && <div class="list-more">向下滚动加载更多</div>}
    </div>
  );
}
