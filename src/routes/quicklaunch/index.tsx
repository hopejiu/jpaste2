import { useState, useCallback, useEffect } from 'preact/hooks';
import { FluentIcon } from '../../components/fluent-icon';
import { api } from '../../lib/invoke';
import { error as logError, setComponent } from '../../lib/logger';
import type { LaunchTarget, LaunchTargetKind } from '../../lib/types';
import { HotkeyEditor } from '../../components/hotkey-editor';

setComponent('quicklaunch');

function generateId(): string {
  return crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

export function QuickLaunchPage() {
  const [targets, setTargets] = useState<LaunchTarget[]>([]);
  const [loading, setLoading] = useState(true);
  const [editingHotkeyId, setEditingHotkeyId] = useState<string | null>(null);
  const [hotkeyErrors, setHotkeyErrors] = useState<Record<string, string>>({});

  const loadTargets = useCallback(async () => {
    try {
      setLoading(true);
      const result = await api.getLaunchTargets();
      setTargets(result);
    } catch (e) {
      logError('get launch targets', e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadTargets(); }, [loadTargets]);

  // Persist the full list (diff-registers hotkeys on the Rust side) and
  // roll back to server state on failure.
  const persist = useCallback(async (updated: LaunchTarget[]) => {
    try {
      await api.saveLaunchTargets(updated);
      setTargets(updated);
    } catch (e) {
      logError('save launch targets', e);
      loadTargets();
    }
  }, [loadTargets]);

  const updateField = useCallback((id: string, patch: Partial<LaunchTarget>) => {
    setTargets((prev) => {
      const updated = prev.map((t) => (t.id === id ? { ...t, ...patch } : t));
      persist(updated);
      return updated;
    });
  }, [persist]);

  const handleNameBlur = useCallback((t: LaunchTarget, value: string) => {
    const name = value.trim();
    if (name && name !== t.name) updateField(t.id, { name });
    else if (!name) loadTargets(); // empty name → revert
  }, [updateField, loadTargets]);

  const handleTargetBlur = useCallback((t: LaunchTarget, value: string) => {
    let target = value.trim();
    if (!target) { loadTargets(); return; }
    if (t.kind === 'web' && !target.includes('://')) target = 'https://' + target;
    if (target !== t.target) updateField(t.id, { target });
  }, [updateField, loadTargets]);

  const handleKindChange = useCallback((id: string, kind: LaunchTargetKind) => {
    updateField(id, { kind });
  }, [updateField]);

  const handleToggle = useCallback((id: string, enabled: boolean) => {
    updateField(id, { enabled });
  }, [updateField]);

  const handleHotkeyChange = useCallback(async (t: LaunchTarget, mods: string[], key: string) => {
    const combo = [...mods, key].join('+');
    try {
      await api.checkTargetHotkey(combo, t.id);
      updateField(t.id, { hotkey: combo });
      setHotkeyErrors((p) => { const n = { ...p }; delete n[t.id]; return n; });
      setEditingHotkeyId(null);
    } catch (e) {
      setHotkeyErrors((p) => ({ ...p, [t.id]: String(e) }));
    }
  }, [updateField]);

  const handleHotkeyClear = useCallback((id: string) => {
    updateField(id, { hotkey: null });
    setHotkeyErrors((p) => { const n = { ...p }; delete n[id]; return n; });
    setEditingHotkeyId(null);
  }, [updateField]);

  const handleAdd = useCallback(() => {
    const t: LaunchTarget = {
      id: generateId(),
      name: '新启动项',
      kind: 'web',
      target: '',
      hotkey: null,
      enabled: true,
    };
    persist([...targets, t]);
  }, [targets, persist]);

  const handleDelete = useCallback((id: string) => {
    persist(targets.filter((t) => t.id !== id));
  }, [targets, persist]);

  const handleTest = useCallback(async (id: string) => {
    try { await api.launchTarget(id); }
    catch (e) { logError('launch target', e); }
  }, []);

  const handlePickFile = useCallback(async (id: string) => {
    const path = await api.pickFilePath();
    if (path) updateField(id, { target: path });
  }, [updateField]);

  return (
    <div class="main-page">
      <div class="ql-page">
        <div class="ql-header">
          <span class="ql-title">快速启动</span>
          <button class="ql-add-btn" onClick={handleAdd} title="添加快捷启动" aria-label="添加">
            <FluentIcon name="add" size={20} />
          </button>
        </div>

        {loading ? (
          <div class="ql-empty">加载中...</div>
        ) : targets.length === 0 ? (
          <div class="ql-empty">
            <p>还没有启动目标</p>
            <p class="ql-empty-hint">点击 + 添加</p>
          </div>
        ) : (
          <div class="ql-list">
            {targets.map((t) => (
              <div key={t.id} class={`ql-item ${!t.enabled ? 'ql-item-disabled' : ''}`}>
                <div class="ql-item-icon" onClick={() => t.enabled && handleTest(t.id)} title="启动">
                  <FluentIcon name={t.kind === 'web' ? 'globe' : 'document'} size={22} />
                </div>
                <div class="ql-item-body">
                  <input
                    class="ql-edit-input ql-edit-name"
                    key={`name-${t.id}-${t.name}`}
                    defaultValue={t.name}
                    placeholder="名称"
                    onBlur={(e) => handleNameBlur(t, (e.target as HTMLInputElement).value)}
                  />
                  <div class="ql-edit-row2">
                    <input
                      class="ql-edit-input ql-edit-target"
                      key={`target-${t.id}-${t.target}`}
                      defaultValue={t.target}
                      placeholder={t.kind === 'web' ? '网址' : '文件路径'}
                      onBlur={(e) => handleTargetBlur(t, (e.target as HTMLInputElement).value)}
                    />
                    {t.kind === 'file' && (
                      <button class="ql-file-btn" onClick={() => handlePickFile(t.id)} title="选择文件" aria-label="选择文件">
                        <FluentIcon name="folder" size={16} />
                      </button>
                    )}
                  </div>
                  <div class="ql-edit-meta">
                    <div class="ql-kind-seg">
                      <button class={`ql-kind-opt ${t.kind === 'web' ? 'active' : ''}`} onClick={() => handleKindChange(t.id, 'web')}>网页</button>
                      <button class={`ql-kind-opt ${t.kind === 'file' ? 'active' : ''}`} onClick={() => handleKindChange(t.id, 'file')}>文件</button>
                    </div>
                    {editingHotkeyId === t.id ? (
                      <div class="ql-hotkey-edit" onClick={(e) => e.stopPropagation()}>
                        <HotkeyEditor
                          hotkey={t.hotkey || ''}
                          error={hotkeyErrors[t.id]}
                          clearable
                          onHotkeyChange={(m, k) => handleHotkeyChange(t, m, k)}
                          onClear={() => handleHotkeyClear(t.id)}
                        />
                      </div>
                    ) : (
                      <button class="ql-hotkey-badge" onClick={() => setEditingHotkeyId(editingHotkeyId === t.id ? null : t.id)} title="点击设置快捷键">
                        {t.hotkey || '设快捷键'}
                      </button>
                    )}
                  </div>
                  {hotkeyErrors[t.id] && editingHotkeyId !== t.id && (
                    <div class="settings-hotkey-error">{hotkeyErrors[t.id]}</div>
                  )}
                </div>
                <div class="ql-item-actions">
                  <label class="ql-toggle" onClick={(e) => e.stopPropagation()}>
                    <input
                      type="checkbox"
                      checked={t.enabled}
                      onChange={(e) => handleToggle(t.id, (e.target as HTMLInputElement).checked)}
                    />
                    <span class="ql-toggle-slider" />
                  </label>
                  <button class="ql-test-btn" onClick={() => handleTest(t.id)} title="测试打开">
                    <FluentIcon name="rocket" size={14} /> 测试
                  </button>
                  <button class="ql-item-btn ql-item-btn-danger" onClick={() => handleDelete(t.id)} title="删除" aria-label="删除">
                    <FluentIcon name="delete" size={16} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
