import { useState, useRef, useCallback } from 'preact/hooks';
import { FluentIcon } from './fluent-icon';

interface QueuePopupProps {
  mode: string;
  items: string[];
  onModeChange: (mode: string) => void;
  onRefreshItems: () => void;
}

/**
 * Queue mode popup component.
 * Shows the current paste queue status and allows switching between normal/queue modes.
 * Manages its own hover state.
 */
export function QueuePopup({ mode, items, onModeChange, onRefreshItems }: QueuePopupProps) {
  const [show, setShow] = useState(false);
  const isQueueMode = mode === 'queue';
  const debounceTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // FP-4: Debounce refresh to avoid rapid API calls on quick mouse movement
  const handleEnter = useCallback(() => {
    setShow(true);
    if (debounceTimer.current) clearTimeout(debounceTimer.current);
    debounceTimer.current = setTimeout(() => {
      onRefreshItems();
    }, 300);
  }, [onRefreshItems]);

  return (
    <div class="bottom-right" onMouseEnter={handleEnter} onMouseLeave={() => setShow(false)}>
      {['normal', 'queue'].map((m) => {
        const active = mode === m;
        return (
          <button
            key={m}
            class={`mode-btn ${active ? 'active' : ''}`}
            onClick={() => onModeChange(m)}
            title={m === 'normal' ? '正常粘贴' : '队列模式：Ctrl+V 顺序粘贴（先进先出）'}
          >
            {m === 'normal' ? '正常' : '队列'}
          </button>
        );
      })}
      {/* Queue popup */}
      {show && isQueueMode && (
        <div class="queue-popup">
          <div class="queue-popup-header">队列 · {items.length} 项</div>
          {items.length === 0 ? (
            <div class="queue-popup-empty">暂无内容</div>
          ) : (
            items.map((item, i) => (
              <div class={`queue-popup-item ${i === 0 ? 'next' : ''}`}>
                <span class="queue-arrow">{i === 0 ? <FluentIcon name="chevronRight" size={12} /> : null}</span>
                <span class="queue-text">{item}</span>
              </div>
            ))
          )}
          <div class="queue-popup-footer"><FluentIcon name="chevronRight" size={12} /> 下一个将粘贴（先进先出）· 复制图片/文件将自动退出队列模式</div>
        </div>
      )}
    </div>
  );
}
