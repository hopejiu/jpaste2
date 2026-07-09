import { useState, useCallback, useEffect, useRef } from 'preact/hooks';
import { FluentIcon } from './fluent-icon';

const MODS = ['Ctrl', 'Alt', 'Shift', 'Win'];

interface HotkeyEditorProps {
  hotkey: string;
  error?: string;
  clearable?: boolean;
  onHotkeyChange: (mods: string[], key: string) => void;
  onClear?: () => void;
}

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

export function HotkeyEditor({ hotkey, error, clearable, onHotkeyChange, onClear }: HotkeyEditorProps) {
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

  const keys = displayKey ? [...parsed.mods, parsed.key].filter(Boolean) : [];

  return (
    <div class="hotkey-editor" ref={containerRef}>
      <div
        class={`hotkey-field ${recording ? 'recording' : ''}`}
        role="button"
        tabIndex={0}
        onClick={() => !recording && startRecording()}
        onKeyDown={(e: KeyboardEvent) => {
          if (!recording && (e.key === 'Enter' || e.key === ' ')) {
            e.preventDefault();
            startRecording();
          }
        }}
      >
        <div class="hotkey-keys">
          {recording ? (
            <span class="hotkey-recording-hint">请按下快捷键组合…</span>
          ) : keys.length > 0 ? (
            keys.map((k) => <kbd class="hotkey-key">{k}</kbd>)
          ) : (
            <span class="hotkey-placeholder">未设置，点击录制</span>
          )}
        </div>

        <div class="hotkey-actions">
          {!recording && (
            <button
              class="hotkey-action"
              type="button"
              title={keys.length > 0 ? '重新录制' : '录制快捷键'}
              onClick={(e) => {
                e.stopPropagation();
                startRecording();
              }}
            >
              <FluentIcon name="edit" size={15} />
            </button>
          )}
          {clearable && keys.length > 0 && !recording && (
            <button
              class="hotkey-action hotkey-clear"
              type="button"
              title="清空快捷键"
              onClick={(e) => {
                e.stopPropagation();
                onClear?.();
              }}
            >
              <FluentIcon name="close" size={15} />
            </button>
          )}
        </div>
      </div>

      {recordError && <div class="settings-hotkey-error">{recordError}</div>}
      {!recording && error && <div class="settings-hotkey-error">{error}</div>}
    </div>
  );
}
