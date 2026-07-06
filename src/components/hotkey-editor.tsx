import { useState, useCallback, useEffect, useRef } from 'preact/hooks';

const MODS = ['Ctrl', 'Alt', 'Shift', 'Win'];

interface HotkeyEditorProps {
  hotkey: string;
  error: string;
  onHotkeyChange: (mods: string[], key: string) => void;
}

const COMMON_PRESETS = [
  { label: 'Alt+V', mods: ['Alt'], key: 'V' },
  { label: 'Ctrl+Shift+V', mods: ['Ctrl', 'Shift'], key: 'V' },
];

function parseHotkey(hotkey: string) {
  const parts = hotkey.split('+').map((p) => p.trim());
  const mods: string[] = [];
  let key = '';
  for (const p of parts) {
    const found = MODS.find((m) => m.toLowerCase() === p.toLowerCase());
    if (found) mods.push(found);
    else key = p;
  }
  return { mods, key };
}

/** Sort modifiers in canonical order: Ctrl, Alt, Shift, Win */
function sortMods(mods: string[]): string[] {
  return [...mods].sort((a, b) => MODS.indexOf(a) - MODS.indexOf(b));
}

export function HotkeyEditor({ hotkey, error, onHotkeyChange }: HotkeyEditorProps) {
  const parsed = parseHotkey(hotkey);
  const [recording, setRecording] = useState(false);
  const [recordError, setRecordError] = useState('');
  const containerRef = useRef<HTMLDivElement>(null);

  const displayKey = [parsed.mods.join('+'), parsed.key].filter(Boolean).join('+');

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    e.preventDefault();
    e.stopPropagation();

    const key = e.key;
    // Ignore standalone modifier keys
    if (['Control', 'Alt', 'Shift', 'Meta'].includes(key)) return;

    const newMods: string[] = [];
    if (e.ctrlKey) newMods.push('Ctrl');
    if (e.altKey) newMods.push('Alt');
    if (e.shiftKey) newMods.push('Shift');
    if (e.metaKey) newMods.push('Win');

    if (newMods.length === 0) {
      setRecordError('至少需要一个修饰键（Ctrl、Alt、Shift、Win）');
      return;
    }

    // Valid: apply immediately
    setRecording(false);
    setRecordError('');
    onHotkeyChange(sortMods(newMods), key.toUpperCase());
  }, [onHotkeyChange]);

  const handleKeyUp = useCallback((e: KeyboardEvent) => {
    if (e.key === 'Escape' && recording) {
      setRecording(false);
      setRecordError('');
    }
  }, [recording]);

  useEffect(() => {
    if (!recording) return;
    // Listen globally so key combos are captured regardless of focus
    document.addEventListener('keydown', handleKeyDown);
    document.addEventListener('keyup', handleKeyUp);
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      document.removeEventListener('keyup', handleKeyUp);
    };
  }, [recording, handleKeyDown, handleKeyUp]);

  const startRecording = useCallback(() => {
    setRecording(true);
    setRecordError('');
  }, []);

  const selectPreset = useCallback((mods: string[], key: string) => {
    onHotkeyChange(mods, key);
  }, [onHotkeyChange]);

  const currentPresetKey = sortMods(parsed.mods).concat(parsed.key ? [parsed.key] : []).join('+');

  return (
    <div class="hotkey-editor" ref={containerRef}>
      <div class={`hotkey-display ${recording ? 'recording' : ''}`}>
        {recording ? (
          <span class="hotkey-recording-hint">请按下快捷键…</span>
        ) : (
          <span class="hotkey-current">{displayKey || '未设置'}</span>
        )}
        <button class="hotkey-record-btn" onClick={startRecording} disabled={recording}>
          {recording ? '监听中' : '录制'}
        </button>
      </div>

      {recordError && <div class="settings-hotkey-error">{recordError}</div>}
      {!recording && error && <div class="settings-hotkey-error">{error}</div>}

      <div class="hotkey-presets">
        <span class="settings-section-desc" style="margin-top:0">常用</span>
        <div class="settings-segment">
          {COMMON_PRESETS.map((preset) => {
            const presetKey = sortMods(preset.mods).concat(preset.key).join('+');
            return (
              <button
                key={presetKey}
                class={`settings-segment-btn ${presetKey === currentPresetKey ? 'active' : ''}`}
                onClick={() => selectPreset(preset.mods, preset.key)}
              >
                {preset.label}
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
