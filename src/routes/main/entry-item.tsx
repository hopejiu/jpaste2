import { useState, useEffect, useRef, useMemo } from 'preact/hooks';
import type { Entry } from '../../lib/types';
import { detect } from '../../actions';
import { formatTime, previewContent } from '../../lib/format';
import { api } from '../../lib/invoke';
import { debug } from '../../lib/logger';
import { convertFileSrc } from '@tauri-apps/api/core';
import { FluentIcon } from '../../components/fluent-icon';
import { TAG_QR } from '../../lib/types';

// FE-5: Shared action icon mapping (single source of truth)
export const ACTION_ICONS: Record<string, string> = {
  'open-url': 'link',
  decoder: 'code',
  json: 'code',
  folder: 'folder',
  curl: 'terminal',
  ws: 'wifi',
  math: 'calculator',
  timestamp: 'clock',
  qrcode: 'qrCode',
};

interface EntryItemProps {
  entry: Entry;
  idx: number;
  focused: boolean;
  onActivate: () => void;
  onDelete: () => void;
  onToggleFav: () => void;
  onOpenEditor: () => void;
  onActionClick?: (actionId: string) => void;
  onImageClick?: () => void;
  onQrClick?: () => void;
  onFocus: () => void;
}

export function EntryItem({
  entry, idx, focused, onActivate, onDelete, onToggleFav,
  onOpenEditor, onActionClick, onImageClick, onQrClick, onFocus,
}: EntryItemProps) {
  const [hovered, setHovered] = useState(false);
  const [imageSrc, setImageSrc] = useState('');
  const showActions = focused || hovered;
  const shortcut = idx < 9 ? `Ctrl+${idx + 1}` : null;
  const time = formatTime(entry.updated_at);
  // FP-1: Cache detect() result to avoid re-running on every render
  // FP-2: Filter out qrcode — list page uses dedicated QR button (modal), not action chip (direct copy)
  const detectedActions = useMemo(() => detect(entry.content, entry.tag_mask).filter(a => a.id !== 'qrcode'), [entry.content, entry.tag_mask]);
  const hasDetected = detectedActions.length > 0;
  const isImage = entry.has_image;
  const hasQr = isImage && (entry.tag_mask & TAG_QR) !== 0;

  // Load image immediately on mount
  useEffect(() => {
    if (!isImage || imageSrc) return;
    debug('EntryItem:loadImage', { entryId: entry.id });
    api.getEntryImage(entry.id)
      .then((filePath) => setImageSrc(convertFileSrc(filePath)))
      .catch((err) => debug('EntryItem:loadImageError', { entryId: entry.id, err }));
  }, [isImage, entry.id]);

  const itemRef = useRef<HTMLDivElement>(null);

  return (
    <div
      ref={itemRef}
      class={`entry-item ${focused ? 'focused' : ''} ${hovered ? 'hovered' : ''}`}
      onMouseEnter={() => { onFocus(); setHovered(true); }}
      onMouseLeave={() => setHovered(false)}
      onClick={onActivate}
    >
      {shortcut && (
        <div class="entry-shortcut">{idx + 1}</div>
      )}

      <div class="entry-body">
        {isImage && imageSrc ? (
          <div class="entry-image-preview" onClick={(e) => { e.stopPropagation(); onImageClick?.(); }}>
            <img src={imageSrc} alt="" class="entry-thumb-img" />
          </div>
        ) : isImage ? (
          <div class="entry-image-placeholder" onClick={(e) => { e.stopPropagation(); onImageClick?.(); }}><FluentIcon name="image" size={20} /></div>
        ) : (
          <div class="entry-content">{previewContent(entry.content)}</div>
        )}

        <div class="entry-meta-row">
          <span class="entry-time">{time.rel}</span>
          <span class="entry-time-abs">{time.abs}</span>
          {entry.copy_count > 0 && (
            <span class="entry-copy-count">· 使用{entry.copy_count}次</span>
          )}

          <div class={`entry-actions ${showActions ? 'visible' : ''}`}>
            {hasDetected && detectedActions.map((act) => {
              const iconName = ACTION_ICONS[act.id];
              return (
                <button
                  key={act.id}
                  class="act-btn"
                  title={act.label}
                  onClick={(e) => { e.stopPropagation(); onActionClick?.(act.id); }}
                >{iconName ? <FluentIcon name={iconName} size={16} /> : act.label}</button>
              );
            })}
            <button
              class={`act-btn ${entry.is_favorite ? 'fav' : ''}`}
              onClick={(e) => { e.stopPropagation(); onToggleFav(); }}
              title={entry.is_favorite ? '取消收藏' : '收藏'}
            >
              <FluentIcon name="star" size={16} filled={entry.is_favorite} />
            </button>
            {hasQr && <button class="act-btn qr" onClick={(e) => { e.stopPropagation(); onQrClick?.(); }} title="二维码" aria-label="二维码"><FluentIcon name="qrCode" size={16} /></button>}
            {!isImage && <button class="act-btn" onClick={(e) => { e.stopPropagation(); onOpenEditor(); }} title="在编辑器中打开" aria-label="在编辑器中打开"><FluentIcon name="edit" size={16} /></button>}
            <button class="act-btn danger" onClick={(e) => { e.stopPropagation(); onDelete(); }} title="删除" aria-label="删除"><FluentIcon name="delete" size={16} /></button>
          </div>
        </div>
      </div>
    </div>
  );
}
