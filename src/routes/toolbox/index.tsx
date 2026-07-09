import { useState, useEffect, useRef } from 'preact/hooks';
import { FluentIcon } from '../../components/fluent-icon';
import { api } from '../../lib/invoke';
import { error as logError, setComponent } from '../../lib/logger';
import { useSidebarTabShortcuts } from '../../hooks/use-sidebar-tabs';

setComponent('toolbox');

function currentBasePath() {
  const hash = window.location.hash;
  const p = hash.startsWith('#') ? hash.slice(1) : hash;
  const quest = p.indexOf('?');
  return quest >= 0 ? p.slice(0, quest) : p;
}

const sidebarItems = [
  { path: '/', icon: 'clipboard' as const, label: '剪贴板' },
  { path: '/toolbox', icon: 'toolbox' as const, label: '工具箱' },
];

const COLUMNS = 3;

interface ToolItem {
  name: string;
  icon: string;
  desc: string;
  action: 'viewer' | 'quicklaunch';
  route?: string;
}

const TOOLS: ToolItem[] = [
  { name: '快速启动', icon: 'rocket', desc: '管理快捷启动目标', action: 'quicklaunch' },
  { name: 'JSON 查看', icon: 'code', desc: 'JSON 格式化与树查看', action: 'viewer', route: '/viewer/json' },
  { name: 'HTTP 调试', icon: 'globe', desc: 'Curl 请求调试器', action: 'viewer', route: '/viewer/curl' },
  { name: 'WS 调试', icon: 'chat', desc: 'WebSocket 调试工具', action: 'viewer', route: '/viewer/ws' },
  { name: '计算器', icon: 'calculator', desc: '表达式计算', action: 'viewer', route: '/viewer/calc' },
  { name: '解码工具', icon: 'lock', desc: 'Base64/URL/Unicode', action: 'viewer', route: '/viewer/decoder' },
  { name: '时间戳转换', icon: 'clock', desc: '时间戳与日期互转', action: 'viewer', route: '/viewer/timestamp' },
  { name: '二维码生成', icon: 'qrCode', desc: '文本/链接生成二维码', action: 'viewer', route: '/viewer/qr' },
  { name: 'SVG 转 PNG', icon: 'image', desc: 'SVG 转 PNG 图片', action: 'viewer', route: '/viewer/svg' },
];

export function ToolboxPage() {
  const basePath = currentBasePath();
  const [pinned, setPinned] = useState(false);
  const [focusedIndex, setFocusedIndex] = useState(0);

  useSidebarTabShortcuts();

  useEffect(() => {
    api.getPinned().then(setPinned).catch((e) => logError('get pinned', e));
  }, []);

  const handleOpen = (tool: ToolItem) => {
    if (tool.action === 'quicklaunch') {
      api.openQuicklaunch().catch((e) => logError('open quicklaunch', e));
      return;
    }
    if (tool.route) {
      api.openViewer(tool.route, -1).catch((e) => logError('open viewer', e));
    }
  };

  // Arrow-key navigation: Left/Right move by 1, Up/Down by a row, Enter opens.
  const handleOpenRef = useRef(handleOpen);
  handleOpenRef.current = handleOpen;
  const focusedRef = useRef(focusedIndex);
  focusedRef.current = focusedIndex;

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const last = TOOLS.length - 1;
      switch (e.key) {
        case 'ArrowRight':
          e.preventDefault();
          setFocusedIndex((i) => Math.min(last, Math.max(0, i) + 1));
          break;
        case 'ArrowLeft':
          e.preventDefault();
          setFocusedIndex((i) => Math.max(0, (i < 0 ? 0 : i) - 1));
          break;
        case 'ArrowDown':
          e.preventDefault();
          setFocusedIndex((i) => Math.min(last, Math.max(0, i) + COLUMNS));
          break;
        case 'ArrowUp':
          e.preventDefault();
          setFocusedIndex((i) => Math.max(0, (i < 0 ? 0 : i) - COLUMNS));
          break;
        case 'Enter': {
          const t = TOOLS[focusedRef.current];
          if (t) {
            e.preventDefault();
            handleOpenRef.current(t);
          }
          break;
        }
      }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, []);

  // Keep the focused card scrolled into view.
  useEffect(() => {
    if (focusedIndex < 0) return;
    const el = document.querySelectorAll('.tb-card')[focusedIndex] as HTMLElement | undefined;
    el?.scrollIntoView({ block: 'nearest' });
  }, [focusedIndex]);

  return (
    <div class="main-page">
      {/* Title Bar */}
      <div class="title-bar" data-tauri-drag-region>
        <div class="title-left">
          {sidebarItems.map((item) => (
            <button
              key={item.path}
              class={`sidebar-btn ${basePath === item.path ? 'active' : ''}`}
              title={item.label}
              onClick={() => window.location.hash = item.path || '/'}
              aria-label={item.label}
            >
              <FluentIcon name={item.icon as any} size={18} />
            </button>
          ))}
        </div>
        <span class="title-text">jPaste</span>
        <div class="title-right">
          <button class="title-btn" title="设置" onClick={() => window.location.hash = '/settings'} aria-label="设置">
            <FluentIcon name="settings" size={18} />
          </button>
          <button
            class={`title-btn ${pinned ? 'active' : ''}`}
            title={pinned ? '取消置顶' : '置顶窗口'}
            onClick={async () => {
              try {
                const newPinned = await api.togglePinned();
                setPinned(newPinned);
              } catch (e) { logError('toggle pinned', e); }}
            }
            aria-label={pinned ? '取消置顶' : '置顶窗口'}
          >
            <FluentIcon name="pin" size={18} filled={pinned} />
          </button>
        </div>
      </div>

      {/* Toolbox Content */}
      <div class="tb-page">
        <div class="tb-header">
          <span class="tb-title">工具箱</span>
        </div>
        <div class="tb-grid">
          {TOOLS.map((tool, i) => (
            <button
              key={tool.name}
              class={`tb-card ${focusedIndex === i ? 'focused' : ''}`}
              onClick={() => { setFocusedIndex(i); handleOpen(tool); }}
              onMouseEnter={() => setFocusedIndex(i)}
              title={tool.desc}
              aria-label={tool.name}
            >
              <FluentIcon name={tool.icon as any} size={28} />
              <span class="tb-card-label">{tool.name}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
