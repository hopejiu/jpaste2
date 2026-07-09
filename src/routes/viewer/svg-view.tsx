import { useState, useEffect, useRef } from 'preact/hooks';
import { FluentIcon } from '../../components/fluent-icon';
import { useViewerEntry } from '../../hooks/use-viewer-entry';
import { api } from '../../lib/invoke';
import { error as logError, setComponent } from '../../lib/logger';

setComponent('svg-view');

/** Decode a base64 string to a plain number[] for Tauri IPC. */
function b64ToBytes(b64: string): number[] {
  const bin = atob(b64);
  const arr = new Array<number>(bin.length);
  for (let i = 0; i < bin.length; i++) arr[i] = bin.charCodeAt(i);
  return arr;
}

/**
 * Layer 1 taint detection: scan the SVG source for external references that
 * would either fail to render or taint the canvas. Returns a reason or null.
 * (Layer 2 is the canvas.toDataURL SecurityError catch during render.)
 */
function detectExternal(svg: string): string | null {
  if (/(?:href|xlink:href|src)\s*=\s*["']\s*https?:/i.test(svg)) return '检测到外部链接引用 (http/https)';
  if (/url\(\s*["']?\s*https?:/i.test(svg)) return '检测到 CSS url() 外部引用';
  if (/@font-face/i.test(svg) && /url\(/i.test(svg)) return '检测到 @font-face 外部字体';
  if (/<foreignObject/i.test(svg)) return '检测到 foreignObject（无法安全栅格化）';
  return null;
}

export function SvgViewPage() {
  const { content } = useViewerEntry();
  const [svg, setSvg] = useState('');
  const [scale, setScale] = useState(1);
  const [preview, setPreview] = useState('');
  const [bytes, setBytes] = useState<number[] | null>(null);
  const [err, setErr] = useState('');
  const [status, setStatus] = useState('');

  useEffect(() => {
    if (content) setSvg(content);
  }, [content]);

  const timer = useRef<number | undefined>(undefined);
  useEffect(() => {
    if (timer.current) clearTimeout(timer.current);
    const src = svg.trim();
    if (!src) { setPreview(''); setBytes(null); setErr(''); return; }
    timer.current = window.setTimeout(() => render(src), 250);
    return () => { if (timer.current) clearTimeout(timer.current); };
  }, [svg, scale]);

  const render = (src: string) => {
    const bad = detectExternal(src);
    if (bad) { setErr(`${bad}，无法转换`); setPreview(''); setBytes(null); return; }

    const blob = new Blob([src], { type: 'image/svg+xml;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const img = new Image();
    img.onload = () => {
      try {
        const w = Math.max(1, Math.round((img.naturalWidth || 300) * scale));
        const h = Math.max(1, Math.round((img.naturalHeight || 300) * scale));
        const canvas = document.createElement('canvas');
        canvas.width = w;
        canvas.height = h;
        const ctx = canvas.getContext('2d');
        if (!ctx) throw new Error('无法创建画布上下文');
        ctx.drawImage(img, 0, 0, w, h);
        URL.revokeObjectURL(url);
        // Throws SecurityError if the canvas was tainted by external resources.
        const dataUrl = canvas.toDataURL('image/png');
        setBytes(b64ToBytes(dataUrl.split(',')[1]));
        setPreview(dataUrl);
        setErr('');
      } catch (e) {
        URL.revokeObjectURL(url);
        setErr('画布被外部资源污染，无法导出 (SecurityError)');
        setPreview('');
        setBytes(null);
      }
    };
    img.onerror = () => {
      URL.revokeObjectURL(url);
      setErr('SVG 解析失败，请检查语法');
      setPreview('');
      setBytes(null);
    };
    img.src = url;
  };

  const flash = (msg: string) => {
    setStatus(msg);
    window.setTimeout(() => setStatus(''), 1600);
  };

  const handleFile = (e: Event) => {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => setSvg(String(reader.result || ''));
    reader.onerror = () => setErr('读取文件失败');
    reader.readAsText(file);
    input.value = ''; // allow re-selecting the same file
  };

  const handleClipboard = () => {
    api.getClipboardText()
      .then((t) => { if (t) setSvg(t); else flash('剪贴板为空'); })
      .catch((e) => { logError('read clipboard', e); flash('读取剪贴板失败'); });
  };

  const handleCopy = () => {
    if (!bytes) return;
    api.writeClipboardImage(bytes)
      .then(() => flash('已复制到剪贴板'))
      .catch((e) => { logError('copy png', e); flash('复制失败'); });
  };

  const handleSave = () => {
    if (!bytes) return;
    api.saveImageDialog(bytes, 'image.png')
      .then((ok) => { if (ok) flash('已保存'); })
      .catch((e) => { logError('save png', e); flash('保存失败'); });
  };

  return (
    <div class="viewer-page">
      <div class="viewer-toolbar" data-tauri-drag-region>
        <div class="viewer-toolbar-icon">
          <FluentIcon name="image" size={20} />
        </div>
        <span class="viewer-title">SVG 转 PNG</span>
        <label class="viewer-btn gen-file-btn">
          读取文件
          <input type="file" accept=".svg,image/svg+xml" onChange={handleFile} hidden />
        </label>
        <button class="viewer-btn" onClick={handleClipboard}>读剪贴板</button>
      </div>

      <div class="viewer-content">
        <div class="viewer-section">
          <textarea
            class="decoder-textarea"
            value={svg}
            onInput={(e) => setSvg((e.target as HTMLTextAreaElement).value)}
            placeholder="粘贴 SVG 源码，或使用上方按钮读取文件/剪贴板..."
            rows={5}
          />
          <div class="gen-row">
            <span class="gen-label">缩放</span>
            <div class="decoder-tabs">
              {[1, 2, 3].map((s) => (
                <button
                  key={s}
                  class={`decoder-tab ${scale === s ? 'active' : ''}`}
                  onClick={() => setScale(s)}
                >{s}x</button>
              ))}
            </div>
          </div>
        </div>

        <div class="viewer-section gen-preview-section">
          {err ? (
            <div class="decoder-error">{err}</div>
          ) : preview ? (
            <img class="gen-preview" src={preview} alt="PNG 预览" />
          ) : (
            <div class="gen-placeholder">预览将显示在这里</div>
          )}
        </div>
      </div>

      <div class="gen-footer">
        <span class="gen-status">{status}</span>
        <button class="viewer-btn" onClick={handleCopy} disabled={!bytes}>
          <FluentIcon name="copy" size={14} /> 复制
        </button>
        <button class="viewer-btn" onClick={handleSave} disabled={!bytes}>保存</button>
      </div>
    </div>
  );
}
