import { useState, useEffect } from 'preact/hooks';
import { FluentIcon } from '../../components/fluent-icon';
import { useEntryId } from '../../hooks/use-entry-id';
import { api } from '../../lib/invoke';
import { copyToClipboard } from '../../lib/clipboard';
import { error as logError } from '../../lib/logger';

interface TimestampResult {
  unix: number;
  iso: string;
  local: string;
  utc: string;
  relative: string;
}

export function TimestampViewPage() {
  const entryId = useEntryId();
  const [input, setInput] = useState('');
  const [result, setResult] = useState<TimestampResult | null>(null);
  const [error, setError] = useState('');

  useEffect(() => {
    if (!entryId) return;
    api.getEntryContent(entryId).then((content) => {
      setInput(content.trim());
      processTimestamp(content.trim());
    }).catch((e) => logError('TimestampViewPage', e));
  }, [entryId]);

  const processTimestamp = (text: string) => {
    setError('');
    if (!text) { setResult(null); return; }

    let ts: number;
    if (/^\d+$/.test(text)) {
      ts = parseInt(text);
      if (text.length === 13) ts = Math.floor(ts / 1000);
    } else {
      const d = new Date(text);
      if (isNaN(d.getTime())) {
        setError('无效的时间戳或日期格式');
        setResult(null);
        return;
      }
      ts = Math.floor(d.getTime() / 1000);
    }

    try {
      const d = new Date(ts * 1000);
      if (isNaN(d.getTime())) { setError('时间戳超出有效范围'); setResult(null); return; }

      const now = Math.floor(Date.now() / 1000);
      const diff = ts - now;
      let relative: string;
      if (Math.abs(diff) < 60) relative = diff >= 0 ? '即将' : '刚刚';
      else if (Math.abs(diff) < 3600) relative = `${Math.floor(diff / 60)} 分钟${diff >= 0 ? '后' : '前'}`;
      else if (Math.abs(diff) < 86400) relative = `${Math.floor(diff / 3600)} 小时${diff >= 0 ? '后' : '前'}`;
      else relative = `${Math.floor(diff / 86400)} 天${diff >= 0 ? '后' : '前'}`;

      setResult({
        unix: ts,
        iso: d.toISOString(),
        // no timeZone — use device locale's default timezone
        local: d.toLocaleString('zh-CN'),
        utc: d.toUTCString(),
        relative,
      });
    } catch (e: any) {
      setError(e.message || '转换失败');
      setResult(null);
    }
  };

  const handleInputChange = (val: string) => {
    setInput(val);
    processTimestamp(val);
  };

  const handleNow = () => {
    const now = Math.floor(Date.now() / 1000);
    setInput(String(now));
    processTimestamp(String(now));
  };

  const handleCopy = (text: string) => {
    copyToClipboard(text);
  };

  return (
    <div class="viewer-page">
      <div class="viewer-toolbar" data-tauri-drag-region>
        <div class="viewer-toolbar-icon">
          <FluentIcon name="clock" size={20} />
        </div>
        <span class="viewer-title">时间戳转换</span>
        <button class="viewer-btn" onClick={handleNow}>
          <FluentIcon name="clock" size={14} /> 现在
        </button>
      </div>
      <div class="viewer-content">
        <div class="viewer-section">
          <div class="viewer-section-title">输入</div>
          <div class="viewer-section-desc">输入 Unix 时间戳或日期字符串</div>
          <div class="ts-input-row">
            <input
              class="ts-input"
              value={input}
              onInput={(e) => handleInputChange((e.target as HTMLInputElement).value)}
              placeholder="输入 Unix 时间戳或日期..."
            />
          </div>
          {error && <div class="ts-error">{error}</div>}
        </div>
        {result && (
          <div class="viewer-section">
            <div class="viewer-section-title">转换结果</div>
            <div class="viewer-section-desc">相对时间: {result.relative}</div>
            <div class="ts-results">
              <div class="ts-row">
                <span class="ts-key">Unix (秒)</span>
                <span class="ts-value">{result.unix}</span>
                <button class="ts-copy" onClick={() => handleCopy(String(result.unix))}><FluentIcon name="copy" size={14} /></button>
              </div>
              <div class="ts-row">
                <span class="ts-key">ISO 8601</span>
                <span class="ts-value">{result.iso}</span>
                <button class="ts-copy" onClick={() => handleCopy(result.iso)}><FluentIcon name="copy" size={14} /></button>
              </div>
              <div class="ts-row">
                <span class="ts-key">本地时间</span>
                <span class="ts-value">{result.local}</span>
                <button class="ts-copy" onClick={() => handleCopy(result.local)}><FluentIcon name="copy" size={14} /></button>
              </div>
              <div class="ts-row">
                <span class="ts-key">UTC</span>
                <span class="ts-value">{result.utc}</span>
                <button class="ts-copy" onClick={() => handleCopy(result.utc)}><FluentIcon name="copy" size={14} /></button>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
