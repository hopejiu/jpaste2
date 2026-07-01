import { useEffect, useState, useCallback, useRef } from 'preact/hooks';
import { api } from '../../lib/invoke';
import { error } from '../../lib/logger';
import type { Settings, Stats } from '../../lib/types';
import { formatBytes } from '../../lib/utils/format';
import { Modal } from '../../components/modal';
import { ToggleSwitch } from '../../components/toggle-switch';
import { HotkeyEditor } from '../../components/hotkey-editor';
import { ActionModuleList } from '../../components/action-module-list';
import { FluentIcon } from '../../components/fluent-icon';

const MODULES_META = [
  { id: 'json', label: 'JSON 查看', desc: '识别 JSON 格式内容', trigger: '内容以 { 或 [ 开头且为合法 JSON' },
  { id: 'folder', label: '打开路径', desc: '识别 Windows 文件路径', trigger: '内容匹配 Windows 路径格式' },
  { id: 'math', label: '计算器', desc: '识别数学表达式', trigger: '内容仅含数字和 +-*/% 运算符' },
  { id: 'decoder', label: '解码工具', desc: 'Base64/URL/Unicode 编解码', trigger: '内容匹配 Base64/URL/Unicode 编码格式' },
  { id: 'open-url', label: '打开链接', desc: '识别 HTTP/FTP/file 协议链接', trigger: '内容以 http:// https:// ftp:// file:// 开头' },
  { id: 'curl', label: 'HTTP 调试', desc: '识别 curl 命令', trigger: '内容以 curl 开头' },
  { id: 'ws', label: 'WS 调试', desc: '识别 WS/WSS 地址', trigger: '内容以 ws:// 或 wss:// 开头' },
  { id: 'timestamp', label: '时间戳转换', desc: '识别 Unix 时间戳', trigger: '内容为纯数字（10位或13位）' },
  { id: 'qrcode', label: '二维码识别', desc: '识别图片中的二维码，支持一键复制', trigger: '剪贴板内容为图片且包含二维码' },
];

export function SettingsPage() {
  const [local, setLocal] = useState<Settings | null>(null);
  const [saved, setSaved] = useState(false);
  const [stats, setStats] = useState<Stats>({ count: 0, total_bytes: 0, image_bytes: 0 });
  const [clearing, setClearing] = useState(false);
  const [showClearModal, setShowClearModal] = useState(false);
  const [hotkeyError, setHotkeyError] = useState('');
  const [showModules, setShowModules] = useState(false);
  const genRef = useRef(0);
  const committedRef = useRef<Settings | null>(null);

  useEffect(() => {
    api.getSettings().then((s) => {
      setLocal(s);
      committedRef.current = s;
      setHotkeyError('');
    });
    api.getStats().then(setStats).catch((e) => error('getStats', e));
  }, []);

  const handleSave = useCallback(async (updates: Partial<Settings>) => {
    if (!local) return;
    const updated = { ...local, ...updates };
    const myGen = ++genRef.current;
    setLocal(updated);
    try {
      await api.saveSettings(updated);
      if (myGen !== genRef.current) return; // superseded by a newer call
      committedRef.current = updated;
      setSaved(true);
      setHotkeyError('');
      setTimeout(() => setSaved(false), 1500);
    } catch (err: any) {
      if (myGen !== genRef.current) return; // superseded by a newer call
      const msg = typeof err === 'string' ? err : err?.message || '保存失败';
      setHotkeyError(msg);
      if (committedRef.current) {
        setLocal(committedRef.current);
      }
    }
  }, [local]);

  const updateHotkey = useCallback((newMods: string[], newKey: string) => {
    const MODS = ['Ctrl', 'Alt', 'Shift', 'Win'];
    const sorted = [...newMods].sort((a, b) => MODS.indexOf(a) - MODS.indexOf(b));
    const hk = newKey ? [...sorted, newKey].join('+') : sorted.join('+');
    handleSave({ hotkey: hk });
  }, [handleSave]);

  const clearHotkey = useCallback(() => {
    handleSave({ hotkey: '' });
  }, [handleSave]);

  // ponytail: action_config no longer has toggle/move UI.
  // These functions were removed when the settings section became read-only reference.
  // If we ever need per-action enable/disable, the props slot is still on Settings.action_config.

  if (!local) return <div class="settings-loading">加载中...</div>;

  return (
    <div class="settings-page">
      {/* Header */}
      <div class="settings-header" data-tauri-drag-region>
        <button class="settings-back" onClick={() => window.location.hash = '/'} aria-label="返回">
          <FluentIcon name="arrowLeft" size={18} />
        </button>
        <h2 class="settings-title">设置</h2>
        {saved && <span class="settings-saved">已保存</span>}
      </div>

      <div class="settings-body">
        {/* Global Hotkey */}
        <div class="settings-section">
          <div class="settings-section-title">全局快捷键</div>
          <div class="settings-section-desc">显示/隐藏 jPaste 窗口</div>
          <HotkeyEditor
            hotkey={local.hotkey}
            error={hotkeyError}
            clearable
            onHotkeyChange={updateHotkey}
            onClear={clearHotkey}
          />
        </div>

        {/* Default Action */}
        <div class="settings-section">
          <div class="settings-section-title">默认操作</div>
          <div class="settings-section-desc">单击条目、按 Enter 或 Ctrl+数字时的默认行为</div>
          <div class="settings-segment">
            <button
              class={`settings-segment-btn ${local.default_action === 'copy' ? 'active' : ''}`}
              onClick={() => {
                handleSave({ default_action: 'copy' });
              }}
            >复制</button>
            <button
              class={`settings-segment-btn ${local.default_action === 'paste' ? 'active' : ''}`}
              onClick={() => {
                handleSave({ default_action: 'paste', auto_hide_after_copy: true });
              }}
            >粘贴</button>
          </div>
        </div>

        {/* Auto-hide after copy — hidden when default action is "paste" (forced true) */}
        {(local.default_action ?? 'copy') !== 'paste' && (
          <div class="settings-section row">
            <div>
              <div class="settings-section-title">复制后自动隐藏</div>
              <div class="settings-section-desc">复制到剪贴板后自动隐藏 jPaste 窗口</div>
            </div>
            <ToggleSwitch
              checked={local.auto_hide_after_copy}
              onChange={() => handleSave({ auto_hide_after_copy: !local.auto_hide_after_copy })}
            />
          </div>
        )}

        {/* Center on show */}
        <div class="settings-section">
          <div class="settings-section-title">唤出位置</div>
          <div class="settings-section-desc">窗口从隐藏变为可见时的定位策略（热键唤出时生效）</div>
          <div class="settings-segment">
            <button
              class={`settings-segment-btn ${local.center_on_show ? 'active' : ''}`}
              onClick={() => handleSave({ center_on_show: true })}
            >居中</button>
            <button
              class={`settings-segment-btn ${!local.center_on_show ? 'active' : ''}`}
              onClick={() => handleSave({ center_on_show: false })}
            >上次位置</button>
          </div>
        </div>

        {/* Auto start */}
        <div class="settings-section row">
          <div>
            <div class="settings-section-title">开机自启</div>
            <div class="settings-section-desc">登录时自动启动 jPaste</div>
          </div>
          <ToggleSwitch
            checked={local.auto_start}
            onChange={async () => {
              const newValue = !local.auto_start;
              handleSave({ auto_start: newValue }).then(async () => {
                try {
                  if (newValue) {
                    await api.enableAutostart();
                  } else {
                    await api.disableAutostart();
                  }
                } catch (e) {
                  error('Failed to update autostart:', e);
                }
              });
            }}
          />
        </div>

        {/* Start minimized */}
        <div class="settings-section">
          <div class="settings-section-title">启动时最小化</div>
          <div class="settings-section-desc">启动后是否显示主窗口</div>
          <div class="settings-segment">
            <button
              class={`settings-segment-btn ${!local.start_minimized ? 'active' : ''}`}
              onClick={() => handleSave({ start_minimized: false })}
            >显示窗口</button>
            <button
              class={`settings-segment-btn ${local.start_minimized ? 'active' : ''}`}
              onClick={() => handleSave({ start_minimized: true })}
            >仅托盘启动</button>
          </div>
        </div>

        {/* Notify */}
        <div class="settings-section row">
          <div>
            <div class="settings-section-title">剪贴板通知</div>
            <div class="settings-section-desc">捕获到新剪贴板内容时显示通知</div>
          </div>
          <ToggleSwitch
            checked={local.notify_enabled}
            onChange={() => handleSave({ notify_enabled: !local.notify_enabled })}
          />
        </div>

        {local.notify_enabled && (
          <div class="settings-section">
            <button
              class="viewer-btn"
              onClick={() => {
                api.showToast('这是一条测试通知消息');
              }}
            >
              预览通知
            </button>
          </div>
        )}

        {/* Retain Days */}
        <div class="settings-section">
          <div class="settings-section-title">保留时长</div>
          <div class="settings-section-desc">超过以下天数的记录自动删除</div>
          <div class="settings-slider-row">
            <input
              type="range" min="1" max="90"
              value={local.retain_days}
              onInput={(e) => setLocal({ ...local, retain_days: parseInt((e.target as HTMLInputElement).value) })}
              onMouseUp={() => handleSave({ retain_days: local.retain_days })}
            />
            <span class="settings-slider-value">{local.retain_days} 天</span>
          </div>
          <div class="settings-stats">
            <span>{stats.count.toLocaleString()} 条记录</span>
            <span class="settings-stats-sep">·</span>
            <span>{formatBytes(stats.total_bytes + stats.image_bytes)}</span>
            {stats.image_bytes > 0 && <span class="settings-stats-sep">·</span>}
            {stats.image_bytes > 0 && <span>图片 {formatBytes(stats.image_bytes)}</span>}
          </div>
          <button
            class="settings-clear-btn"
            onClick={() => setShowClearModal(true)}
            disabled={clearing || stats.count === 0}
          >
            {clearing ? '清空中...' : '清空全部历史'}
          </button>
        </div>

        {/* Auto Favorite on Copy Count */}
        <div class="settings-section">
          <div class="settings-section-title">高频自动收藏</div>
          <div class="settings-section-desc">当条目使用次数达到阈值时自动收藏，防止被自动清理</div>
          <div class="settings-section row" style="margin-top: 8px;">
            <span>达到阈值时自动收藏</span>
            <ToggleSwitch
              checked={local.auto_fav_on_copy_count}
              onChange={() => handleSave({ auto_fav_on_copy_count: !local.auto_fav_on_copy_count })}
            />
          </div>
          {local.auto_fav_on_copy_count && (
            <div class="settings-slider-row">
              <input
                type="range" min="2" max="10"
                value={local.auto_fav_threshold}
                onInput={(e) => setLocal({ ...local, auto_fav_threshold: parseInt((e.target as HTMLInputElement).value) })}
                onMouseUp={() => handleSave({ auto_fav_threshold: local.auto_fav_threshold })}
              />
              <span class="settings-slider-value">{local.auto_fav_threshold} 次</span>
            </div>
          )}
        </div>

        {/* Action Modules — collapsed read-only reference */}
        <div class="settings-section">
          <div
            class="settings-collapse-header"
            onClick={() => setShowModules(!showModules)}
            role="button"
            tabIndex={0}
            onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setShowModules(!showModules); } }}
          >
            <div>
              <div class="settings-section-title">检测能力 · 通知增强</div>
              <div class="settings-section-desc">复制内容时自动检测格式，在通知右下角显示可点击的动作按钮</div>
            </div>
            <span class={`collapse-arrow ${showModules ? 'open' : ''}`}>▶</span>
          </div>
          {showModules && <ActionModuleList modules={MODULES_META} />}
        </div>
      </div>

      {/* Clear All Confirmation Modal */}
      <Modal open={showClearModal} onClose={() => setShowClearModal(false)} title="清空剪贴板历史">
        <p class="clear-modal-text">
          共有 <strong>{stats.count.toLocaleString()}</strong> 条记录。选择清空方式：
        </p>
        <div class="clear-modal-actions">
          <button
            class="clear-modal-btn"
            onClick={async () => {
              setShowClearModal(false);
              setClearing(true);
              try { await api.clearAll(false); const s = await api.getStats(); setStats(s); } catch (e) { error('clearAll failed', e); }
              setClearing(false);
            }}
          >
            <div class="clear-modal-btn-title">全部删除</div>
            <div class="clear-modal-btn-desc">删除所有记录（包括收藏），不可撤销</div>
          </button>
          <button
            class="clear-modal-btn"
            onClick={async () => {
              setShowClearModal(false);
              setClearing(true);
              try { await api.clearAll(true); const s = await api.getStats(); setStats(s); } catch (e) { error('clearAll failed', e); }
              setClearing(false);
            }}
          >
            <div class="clear-modal-btn-title">保留收藏</div>
            <div class="clear-modal-btn-desc">只删除未收藏的记录，收藏内容保留</div>
          </button>
          <button class="clear-modal-cancel" onClick={() => setShowClearModal(false)}>取消</button>
        </div>
      </Modal>
    </div>
  );
}
