import { useState, useEffect } from 'preact/hooks';
import { FluentIcon } from '../../components/fluent-icon';
import { useViewerEntry } from '../../hooks/use-viewer-entry';
import { copyToClipboard } from '../../lib/clipboard';
import { relativeTime } from '../../lib/format';


interface TimestampResult {
  unix: number;
  unixMs: number;
  iso: string;
  local: string;
  utc: string;
  relative: string;
}

type Mode = 'ts-to-date' | 'date-to-ts';

export function TimestampViewPage() {
  const { content } = useViewerEntry();
  const [mode, setMode] = useState<Mode>('ts-to-date');
  const [input, setInput] = useState('');
  const [result, setResult] = useState<TimestampResult | null>(null);
  const [error, setError] = useState('');

  // Date-to-ts inputs
  const now = new Date();
  const [year, setYear] = useState(String(now.getFullYear()));
  const [month, setMonth] = useState(String(now.getMonth() + 1).padStart(2, '0'));
  const [day, setDay] = useState(String(now.getDate()).padStart(2, '0'));
  const [hour, setHour] = useState(String(now.getHours()).padStart(2, '0'));
  const [minute, setMinute] = useState(String(now.getMinutes()).padStart(2, '0'));
  const [second, setSecond] = useState(String(now.getSeconds()).padStart(2, '0'));

  useEffect(() => {
    if (!content) return; // blank viewer for toolbox
    const trimmed = content.trim();
    setInput(trimmed);
    processTimestamp(trimmed);
  }, [content]);

  // Process date-to-ts when any component changes
  useEffect(() => {
    if (mode !== 'date-to-ts') return;
    const y = parseInt(year), m = parseInt(month), d = parseInt(day);
    const h = parseInt(hour), min = parseInt(minute), s = parseInt(second);
    if ([y, m, d, h, min, s].some(isNaN)) return;
    const date = new Date(y, m - 1, d, h, min, s);
    if (isNaN(date.getTime())) { setError('无效日期'); setResult(null); return; }
    const ts = Math.floor(date.getTime() / 1000);
    buildResult(ts);
  }, [mode, year, month, day, hour, minute, second]);

  const buildResult = (ts: number) => {
    setError('');
    try {
      const d = new Date(ts * 1000);
      if (isNaN(d.getTime())) { setError('时间戳超出有效范围'); setResult(null); return; }
      setResult({
        unix: ts,
        unixMs: ts * 1000,
        iso: d.toISOString(),
        local: d.toLocaleString('zh-CN'),
        utc: d.toUTCString(),
        relative: relativeTime(ts * 1000),
      });
    } catch (e: any) {
      setError(e.message || '转换失败');
      setResult(null);
    }
  };

  const processTimestamp = (text: string) => {
    setError('');
    if (!text) { setResult(null); return; }
    let ts: number;
    if (/^\d+$/.test(text)) {
      ts = parseInt(text);
      if (text.length === 13) ts = Math.floor(ts / 1000);
    } else {
      const d = new Date(text);
      if (isNaN(d.getTime())) { setError('无效的时间戳或日期格式'); setResult(null); return; }
      ts = Math.floor(d.getTime() / 1000);
    }
    buildResult(ts);
  };

  const handleInputChange = (val: string) => {
    setInput(val);
    processTimestamp(val);
  };

  const handleNow = () => {
    if (mode === 'ts-to-date') {
      const nowTs = Math.floor(Date.now() / 1000);
      setInput(String(nowTs));
      processTimestamp(String(nowTs));
    } else {
      const n = new Date();
      setYear(String(n.getFullYear()));
      setMonth(String(n.getMonth() + 1).padStart(2, '0'));
      setDay(String(n.getDate()).padStart(2, '0'));
      setHour(String(n.getHours()).padStart(2, '0'));
      setMinute(String(n.getMinutes()).padStart(2, '0'));
      setSecond(String(n.getSeconds()).padStart(2, '0'));
    }
  };

  const handleCopy = (text: string) => {
    copyToClipboard(text);
  };

  const switchMode = (m: Mode) => {
    setMode(m);
    setResult(null);
    setError('');
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
        {/* Mode toggle */}
        <div class="viewer-section">
          <div class="ts-mode-toggle">
            <button
              class={`ts-mode-btn ${mode === 'ts-to-date' ? 'active' : ''}`}
              onClick={() => switchMode('ts-to-date')}
            >时间戳 → 日期</button>
            <button
              class={`ts-mode-btn ${mode === 'date-to-ts' ? 'active' : ''}`}
              onClick={() => switchMode('date-to-ts')}
            >日期 → 时间戳</button>
          </div>
        </div>

        {/* Input area — changes by mode */}
        {mode === 'ts-to-date' ? (
          <div class="viewer-section">
            <div class="viewer-section-title">输入时间戳</div>
            <div class="viewer-section-desc">输入 Unix 时间戳（10 位秒 / 13 位毫秒）或日期字符串</div>
            <div class="ts-input-row">
              <input
                class="ts-input"
                value={input}
                onInput={(e) => handleInputChange((e.target as HTMLInputElement).value)}
                placeholder="例如: 1720000000 或 2024-07-03 12:00:00"
              />
            </div>
            {error && <div class="ts-error">{error}</div>}
          </div>
        ) : (
          <div class="viewer-section">
            <div class="viewer-section-title">输入日期</div>
            <div class="viewer-section-desc">年月日时分秒 → 时间戳</div>
            <div class="ts-date-grid">
              <div class="ts-date-field">
                <label class="ts-date-label">年</label>
                <input class="ts-date-input" value={year} onInput={(e) => setYear((e.target as HTMLInputElement).value)} placeholder="2024" />
              </div>
              <div class="ts-date-field">
                <label class="ts-date-label">月</label>
                <input class="ts-date-input" value={month} onInput={(e) => setMonth((e.target as HTMLInputElement).value)} placeholder="01-12" />
              </div>
              <div class="ts-date-field">
                <label class="ts-date-label">日</label>
                <input class="ts-date-input" value={day} onInput={(e) => setDay((e.target as HTMLInputElement).value)} placeholder="01-31" />
              </div>
              <div class="ts-date-field">
                <label class="ts-date-label">时</label>
                <input class="ts-date-input" value={hour} onInput={(e) => setHour((e.target as HTMLInputElement).value)} placeholder="00-23" />
              </div>
              <div class="ts-date-field">
                <label class="ts-date-label">分</label>
                <input class="ts-date-input" value={minute} onInput={(e) => setMinute((e.target as HTMLInputElement).value)} placeholder="00-59" />
              </div>
              <div class="ts-date-field">
                <label class="ts-date-label">秒</label>
                <input class="ts-date-input" value={second} onInput={(e) => setSecond((e.target as HTMLInputElement).value)} placeholder="00-59" />
              </div>
            </div>
            {error && <div class="ts-error">{error}</div>}
          </div>
        )}

        {/* Results */}
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
                <span class="ts-key">Unix (毫秒)</span>
                <span class="ts-value">{result.unixMs}</span>
                <button class="ts-copy" onClick={() => handleCopy(String(result.unixMs))}><FluentIcon name="copy" size={14} /></button>
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
