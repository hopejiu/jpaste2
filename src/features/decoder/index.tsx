import { useState, useEffect } from 'preact/hooks';
import { FluentIcon } from '../../components/fluent-icon';
import { useViewerEntry } from '../../hooks/use-viewer-entry';
import { copyToClipboard } from '../../lib/clipboard';

type DecodeMode = 'base64' | 'url' | 'unicode' | 'escape';
const MODE_ORDER: DecodeMode[] = ['base64', 'url', 'unicode', 'escape'];

/* ── encode / decode primitives (UTF-8 aware) ── */

function utf8ToBase64(str: string): string {
  const bytes = new TextEncoder().encode(str);
  let bin = '';
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}

function base64ToUtf8(b64: string): string {
  // Tolerate url-safe alphabet and stray whitespace.
  const norm = b64.replace(/-/g, '+').replace(/_/g, '/').replace(/\s/g, '');
  const bin = atob(norm);
  const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}

function encodeUnicode(str: string): string {
  let out = '';
  for (const ch of str) {
    const cp = ch.codePointAt(0)!;
    out += cp <= 0xffff
      ? '\\u' + cp.toString(16).padStart(4, '0')
      : '\\u{' + cp.toString(16) + '}';
  }
  return out;
}

function decodeUnicode(str: string): string {
  return str
    .replace(/\\u\{([0-9a-fA-F]+)\}/g, (_, h) => String.fromCodePoint(parseInt(h, 16)))
    .replace(/\\u([0-9a-fA-F]{4})/g, (_, h) => String.fromCharCode(parseInt(h, 16)))
    .replace(/\\x([0-9a-fA-F]{2})/g, (_, h) => String.fromCharCode(parseInt(h, 16)));
}

/* ── escape / unescape (C/JSON-style backslash sequences) ── */

function encodeEscape(str: string): string {
  let out = '';
  for (const ch of str) {
    switch (ch) {
      case '\\': out += '\\\\'; break;
      case '"': out += '\\"'; break;
      case "'": out += "\\'"; break;
      case '\n': out += '\\n'; break;
      case '\t': out += '\\t'; break;
      case '\r': out += '\\r'; break;
      case '\b': out += '\\b'; break;
      case '\f': out += '\\f'; break;
      case '/': out += '\\/'; break;
      default: out += ch;
    }
  }
  return out;
}

// ponytail: manual left-to-right scan so `\\n` decodes to backslash+n, not newline.
function decodeEscape(str: string): string {
  let out = '';
  for (let i = 0; i < str.length; i++) {
    if (str[i] === '\\' && i + 1 < str.length) {
      const n = str[i + 1];
      switch (n) {
        case 'n': out += '\n'; break;
        case 't': out += '\t'; break;
        case 'r': out += '\r'; break;
        case 'b': out += '\b'; break;
        case 'f': out += '\f'; break;
        case '"': out += '"'; break;
        case "'": out += "'"; break;
        case '/': out += '/'; break;
        case '\\': out += '\\'; break;
        case '0': out += '\0'; break;
        default: out += n; // unknown escape: keep following char literally
      }
      i++;
    } else {
      out += str[i];
    }
  }
  return out;
}

function isLikelyEscaped(s: string): boolean {
  return (s.match(/\\(["'ntrb\/\\])/g) || []).length >= 2;
}

function encodeText(text: string, mode: DecodeMode): string {
  switch (mode) {
    case 'base64': return utf8ToBase64(text);
    case 'url': return encodeURIComponent(text);
    case 'unicode': return encodeUnicode(text);
    case 'escape': return encodeEscape(text);
  }
}

function decodeText(text: string, mode: DecodeMode): string {
  switch (mode) {
    case 'base64': return base64ToUtf8(text);
    case 'url': return decodeURIComponent(text);
    case 'unicode': return decodeUnicode(text);
    case 'escape': return decodeEscape(text);
  }
}

/* ── smart detection (used when loading clip/toolbox content) ── */

function isLikelyBase64(s: string): boolean {
  if (s.length < 4) return false;
  if (s.length % 4 === 1) return false; // invalid base64 length
  return /^[A-Za-z0-9+/=]+$/.test(s);
}

function detectMode(text: string): DecodeMode {
  const s = text.trim();
  if (/%[0-9a-fA-F]{2}/.test(s)) return 'url';
  if (/\\u[0-9a-fA-F]{4}/.test(s) || /\\x[0-9a-fA-F]{2}/.test(s)) return 'unicode';
  if (isLikelyEscaped(s)) return 'escape';
  if (isLikelyBase64(s)) return 'base64';
  return 'base64';
}

export function DecoderViewPage() {
  const { content } = useViewerEntry();
  const [mode, setMode] = useState<DecodeMode>('base64');
  const [top, setTop] = useState('');       // 原文
  const [bottom, setBottom] = useState(''); // 编码后
  const [error, setError] = useState('');
  const [errorSide, setErrorSide] = useState<'top' | 'bottom' | null>(null);
  const [copiedSide, setCopiedSide] = useState<'top' | 'bottom' | null>(null);

  // Load clip/toolbox content: smart-detect mode, then pick the source side.
  useEffect(() => {
    if (!content) return; // blank viewer for toolbox
    const detected = detectMode(content);
    setMode(detected);
    try {
      const t = content.trim();
      const looksEncoded =
        (detected === 'url' && /%[0-9a-fA-F]{2}/.test(t)) ||
        (detected === 'unicode' && /\\u[0-9a-fA-F]{4}/.test(t)) ||
        (detected === 'escape' && isLikelyEscaped(t)) ||
        (detected === 'base64' && isLikelyBase64(t) && t.length >= 8);
      if (looksEncoded) {
        setBottom(content);
        setTop(decodeText(content, detected));
      } else {
        setTop(content);
        setBottom(encodeText(content, detected));
      }
      setError('');
      setErrorSide(null);
    } catch {
      // Fallback: show as 原文 and encode.
      setTop(content);
      setBottom(encodeText(content, detected));
      setError('');
      setErrorSide(null);
    }
  }, [content]);

  const handleTopInput = (val: string) => {
    setTop(val);
    setError('');
    setErrorSide(null);
    try {
      setBottom(encodeText(val, mode));
    } catch (e: any) {
      setError(e?.message || '编码失败');
      setErrorSide('bottom');
    }
  };

  const handleBottomInput = (val: string) => {
    setBottom(val);
    setError('');
    setErrorSide(null);
    try {
      setTop(decodeText(val, mode));
    } catch (e: any) {
      setError(e?.message || '解码失败');
      setErrorSide('top');
    }
  };

  const handleModeChange = (m: DecodeMode) => {
    setMode(m);
    // 原文(top) is the canonical source → re-encode into bottom.
    setError('');
    setErrorSide(null);
    try {
      setBottom(encodeText(top, m));
    } catch (e: any) {
      setError(e?.message || '编码失败');
      setErrorSide('bottom');
    }
  };

  const handleSwap = () => {
    // Exchange top & bottom contents (原文 ↔ 编码后).
    setTop(bottom);
    setBottom(top);
    setError('');
    setErrorSide(null);
  };

  const handleCopyText = async (text: string, side: 'top' | 'bottom') => {
    if (!text) return;
    const ok = await copyToClipboard(text);
    if (ok) {
      setCopiedSide(side);
      window.setTimeout(() => setCopiedSide((s) => (s === side ? null : s)), 1500);
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    const mod = e.ctrlKey || e.metaKey;
    if (e.key === 'Enter' && mod) {
      e.preventDefault();
      handleSwap();
    } else if (e.key === 'Enter') {
      // Enter copies the encoded result; Shift+Enter inserts a newline.
      if (e.shiftKey) return;
      e.preventDefault();
      handleCopyText(bottom, 'bottom');
    } else if (e.key === 'Tab') {
      e.preventDefault();
      const next = MODE_ORDER[(MODE_ORDER.indexOf(mode) + 1) % MODE_ORDER.length];
      handleModeChange(next);
    }
  };

  return (
    <div class="viewer-page">
      <div class="viewer-toolbar" data-tauri-drag-region>
        <div class="viewer-toolbar-icon">
          <FluentIcon name="code" size={20} />
        </div>
        <span class="viewer-title">解码工具</span>
        <button class="viewer-btn sm" onClick={handleSwap} title="交换上下内容 (Ctrl/Cmd+Enter)">
          <FluentIcon name="arrowLeft" size={14} /> 交换
        </button>
      </div>
      <div class="viewer-content">
        <div class="viewer-section" onKeyDown={handleKeyDown}>
          <div class="decoder-modes">
            {MODE_ORDER.map((m) => (
              <button
                class={`decoder-mode-tab ${mode === m ? 'active' : ''}`}
                onClick={() => handleModeChange(m)}
              >
                {m === 'base64' ? 'Base64' : m === 'url' ? 'URL' : m === 'unicode' ? 'Unicode' : 'Escape'}
              </button>
            ))}
            <div class="decoder-modes-spacer" />
          </div>

          <div class="decoder-panels">
            <div class="decoder-panel">
              <div class="decoder-panel-head">
                <span class="decoder-panel-label">原文</span>
                <div class="decoder-panel-head-right">
                  <span class="decoder-panel-hint">编辑自动编码 ↓</span>
                  {errorSide === 'top' && <span class="decoder-err-tag">解码错误</span>}
                </div>
              </div>
              <div class="decoder-field-wrap">
                <textarea
                  class={`decoder-field ${errorSide === 'top' ? 'is-error' : ''}`}
                  value={top}
                  onInput={(e) => handleTopInput((e.target as HTMLTextAreaElement).value)}
                  placeholder="在此输入原文…"
                />
                <button
                  class="decoder-copy-btn"
                  onClick={() => handleCopyText(top, 'top')}
                  disabled={!top}
                  title="复制原文"
                >
                  <FluentIcon name="copy" size={13} />
                  {copiedSide === 'top' ? '已复制' : '复制'}
                </button>
              </div>
            </div>

            <div class="decoder-panel">
              <div class="decoder-panel-head">
                <span class="decoder-panel-label">编码后</span>
                <div class="decoder-panel-head-right">
                  <span class="decoder-panel-hint">编辑自动解码 ↑</span>
                  {errorSide === 'bottom' && <span class="decoder-err-tag">编码错误</span>}
                </div>
              </div>
              <div class="decoder-field-wrap">
                <textarea
                  class={`decoder-field ${errorSide === 'bottom' ? 'is-error' : ''}`}
                  value={bottom}
                  onInput={(e) => handleBottomInput((e.target as HTMLTextAreaElement).value)}
                  placeholder="编码结果将显示在这里…"
                />
                <button
                  class="decoder-copy-btn"
                  onClick={() => handleCopyText(bottom, 'bottom')}
                  disabled={!bottom}
                  title="复制编码结果"
                >
                  <FluentIcon name="copy" size={13} />
                  {copiedSide === 'bottom' ? '已复制' : '复制'}
                </button>
              </div>
            </div>
          </div>

          {error && <div class="decoder-error">{error}</div>}
        </div>
      </div>
    </div>
  );
}
