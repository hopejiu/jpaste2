import { useEffect, useState, useRef } from 'preact/hooks';
import { listen } from '@tauri-apps/api/event';
import { FluentIcon } from '../components/fluent-icon';
import { info, setComponent } from '../lib/logger';
import { TOAST_SHOW } from '../lib/events';
import { api } from '../lib/invoke';

// ponytail: mirrors entry-item.tsx ACTION_ICONS + action labels.
// Kept here independently so the toast window doesn't pull in the full
// actions/ module (which registers all action modules as side effects).
const ACTION_META: Record<string, { icon: string; label: string }> = {
  json:      { icon: 'code',      label: 'JSON 查看' },
  curl:      { icon: 'terminal',  label: 'HTTP 调试' },
  ws:        { icon: 'wifi',      label: 'WS 调试' },
  decoder:   { icon: 'code',      label: '解码' },
  timestamp: { icon: 'clock',     label: '时间戳转换' },
  math:      { icon: 'calculator',label: '计算' },
  folder:    { icon: 'folder',    label: '打开所在目录' },
  'open-url':{ icon: 'link',      label: '打开链接' },
  qrcode:    { icon: 'qrCode',    label: '复制二维码' },
};

const VIEWER_ROUTES: Record<string, string> = {
  json: '/viewer/json',
  curl: '/viewer/curl',
  ws: '/viewer/ws',
  decoder: '/viewer/decoder',
  timestamp: '/viewer/timestamp',
  math: '/viewer/calc',
};

// Ensure action names match Rust side detect_actions() output
type ActionId = keyof typeof ACTION_META;

/**
 * Toast page — displayed in a small frameless Tauri window.
 * The window is auto-closed from Rust 3 seconds after creation
 * (see `create_toast_window_inner` in lib.rs).
 */
export function ToastPage() {
  setComponent('toast');
  const [title, setTitle] = useState('jPaste');
  const [message, setMessage] = useState('');
  const [icon, setIcon] = useState('clipboard');
  const [actions, setActions] = useState<string[]>([]);
  const [entryId, setEntryId] = useState<number>(0);
  const [fullText, setFullText] = useState('');
  const containerRef = useRef<HTMLDivElement>(null);
  const cardRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    // Listen for content updates from Rust (window reuse path)
    listen<{
      title?: string; message?: string; icon?: string;
      id?: number; text?: string; actions?: string[];
    }>(TOAST_SHOW, (event) => {
      const { title, message, icon, id, text, actions } = event.payload;
      if (title) setTitle(title);
      if (message !== undefined) setMessage(message);
      if (icon) setIcon(icon);
      if (id !== undefined) setEntryId(id);
      if (text !== undefined) setFullText(text);
      if (actions !== undefined) setActions(actions);
    }).then((fn) => { unlisten = fn; });

    // Initial load from hash (first mount, new window path)
    const params = new URLSearchParams(window.location.hash.split('?')[1] ?? '');
    setTitle(params.get('title') ?? 'jPaste');
    setMessage(params.get('message') ?? '');
    setIcon(params.get('icon') ?? 'clipboard');
    setEntryId(Number(params.get('id')) || 0);
    setFullText(params.get('text') ?? '');

    const rawActions = params.get('actions');
    if (rawActions) {
      setActions(rawActions.split(',').filter(Boolean));
    }

    info('page loaded');
    return () => { unlisten?.(); };
  }, []);

  const handleActionClick = (actionId: string) => {
    const meta = ACTION_META[actionId as ActionId];
    if (!meta) return;

    if (actionId === 'qrcode') {
      // Toast "qrcode" — fetch QR text and copy to clipboard
      api.scanQrText(entryId).then((text) => {
        if (text) navigator.clipboard.writeText(text).catch(() => {});
      }).catch(() => {});
    } else if (actionId === 'folder') {
      // Toast "folder" opens the parent directory (useful for file copies from Explorer).
      // For isolated directory paths the user would use EntryItem's folder modal.
      const idx = fullText.lastIndexOf('\\');
      const dir = idx >= 0 ? fullText.substring(0, idx) : fullText;
      api.invoke('open_in_explorer', { path: dir }).catch(() => {});
    } else if (actionId === 'open-url') {
      api.openUrl(fullText).catch(() => {});
    } else {
      const route = VIEWER_ROUTES[actionId];
      if (route && entryId > 0) {
        api.openViewer(route, entryId).catch(() => {});
      }
    }
  };

  const hasActions = actions.length > 0;

  return (
    <div
      ref={containerRef}
      class="toast-container"
    >
      <div ref={cardRef} class="toast-card">
        <div class="toast-icon"><FluentIcon name={icon} size={20} /></div>
        <div class="toast-content">
          <div class="toast-title">{title}</div>
          <div class="toast-message">{message}</div>
        </div>
      </div>

      {hasActions && (
        <>
          <div class="toast-divider" />
          <div class="toast-actions">
            {actions.map((id) => {
              const meta = ACTION_META[id as ActionId];
              if (!meta) return null;
              return (
                <button
                  key={id}
                  class="toast-action-chip"
                  onClick={(e) => {
                    e.stopPropagation();
                    handleActionClick(id);
                  }}
                  title={meta.label}
                >
                  <FluentIcon name={meta.icon} size={14} />
                  <span>{meta.label}</span>
                </button>
              );
            })}
          </div>
        </>
      )}
    </div>
  );
}
