import { useState, useEffect, useRef } from 'preact/hooks';
import { FluentIcon } from '../../components/fluent-icon';
import { useViewerEntry } from '../../hooks/use-viewer-entry';
import { api } from '../../lib/invoke';
import { error as logError, setComponent } from '../../lib/logger';

setComponent('qr-view');

const EC_LEVELS = ['L', 'M', 'Q', 'H'] as const;
type EcLevel = (typeof EC_LEVELS)[number];

/** Decode a base64 string to a plain number[] for Tauri IPC. */
function b64ToBytes(b64: string): number[] {
  const bin = atob(b64);
  const arr = new Array<number>(bin.length);
  for (let i = 0; i < bin.length; i++) arr[i] = bin.charCodeAt(i);
  return arr;
}

export function QrViewPage() {
  const { content } = useViewerEntry();
  const [text, setText] = useState('');
  const [size, setSize] = useState(320);
  const [maxSize, setMaxSize] = useState(320);
  const [ecLevel, setEcLevel] = useState<EcLevel>('M');
  const [margin, setMargin] = useState(2);
  const [fg, setFg] = useState('#000000');
  const [bg, setBg] = useState('#ffffff');
  const [preview, setPreview] = useState('');
  const [err, setErr] = useState('');
  const [status, setStatus] = useState('');
  const previewRef = useRef<HTMLDivElement>(null);

  // Slider max follows the largest square that fits the preview area, so
  // dragging to the top fills the container exactly. Recomputed on resize.
  useEffect(() => {
    const el = previewRef.current;
    if (!el) return;
    const measure = () => {
      const cs = getComputedStyle(el);
      const padX = parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight);
      const padY = parseFloat(cs.paddingTop) + parseFloat(cs.paddingBottom);
      const w = el.clientWidth - padX;
      const h = el.clientHeight - padY;
      setMaxSize(Math.max(80, Math.floor(Math.min(w, h))));
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Keep size within the (window-derived) max when the container shrinks.
  useEffect(() => {
    if (size > maxSize) setSize(maxSize);
  }, [maxSize, size]);

  // Prefill from a clipboard entry if opened with one.
  useEffect(() => {
    if (content) setText(content);
  }, [content]);

  // Debounced real-time generation. `seq` drops stale responses so slow drags
  // that fire several async requests can't set an older (smaller/larger) result.
  const timer = useRef<number | undefined>(undefined);
  const seq = useRef(0);
  useEffect(() => {
    if (timer.current) clearTimeout(timer.current);
    if (!text.trim()) {
      setPreview('');
      setErr('');
      return;
    }
    const id = ++seq.current;
    timer.current = window.setTimeout(() => {
      api
        .generateQr({ content: text, size, ecLevel, margin, fg, bg })
        .then((b64) => {
          if (id !== seq.current) return;
          setPreview(b64);
          setErr('');
        })
        .catch((e) => {
          if (id !== seq.current) return;
          setPreview('');
          setErr(String(e?.message || e || '生成失败'));
        });
    }, 200);
    return () => { if (timer.current) clearTimeout(timer.current); };
  }, [text, size, ecLevel, margin, fg, bg]);

  const flash = (msg: string) => {
    setStatus(msg);
    window.setTimeout(() => setStatus(''), 1600);
  };

  const handleCopy = () => {
    if (!preview) return;
    api.writeClipboardImage(b64ToBytes(preview))
      .then(() => flash('已复制到剪贴板'))
      .catch((e) => { logError('copy qr', e); flash('复制失败'); });
  };

  const handleSave = () => {
    if (!preview) return;
    api.saveImageDialog(b64ToBytes(preview), 'qrcode.png')
      .then((ok) => { if (ok) flash('已保存'); })
      .catch((e) => { logError('save qr', e); flash('保存失败'); });
  };

  return (
    <div class="viewer-page">
      <div class="viewer-toolbar" data-tauri-drag-region>
        <div class="viewer-toolbar-icon">
          <FluentIcon name="qrCode" size={20} />
        </div>
        <span class="viewer-title">二维码生成</span>
      </div>

      <div class="viewer-content">
        <div class="viewer-section gen-form-section">
          <textarea
            class="decoder-textarea"
            value={text}
            onInput={(e) => setText((e.target as HTMLTextAreaElement).value)}
            placeholder="输入文本或链接..."
            rows={2}
          />

          <div class="gen-form">
            <label class="gen-row">
              <span class="gen-label">尺寸</span>
              <input
                type="range" min={Math.min(160, maxSize)} max={maxSize} step={10}
                value={Math.min(size, maxSize)}
                onInput={(e) => setSize(Number((e.target as HTMLInputElement).value))}
              />
              <span class="gen-value">{Math.min(size, maxSize)}px</span>
            </label>

            <label class="gen-row">
              <span class="gen-label">边距</span>
              <input
                type="range" min={0} max={8} step={1}
                value={margin}
                onInput={(e) => setMargin(Number((e.target as HTMLInputElement).value))}
              />
              <span class="gen-value">{margin}</span>
            </label>

            <div class="gen-row">
              <span class="gen-label">纠错</span>
              <div class="decoder-tabs">
                {EC_LEVELS.map((lv) => (
                  <button
                    key={lv}
                    class={`decoder-tab ${ecLevel === lv ? 'active' : ''}`}
                    onClick={() => setEcLevel(lv)}
                  >{lv}</button>
                ))}
              </div>
            </div>

            <div class="gen-row">
              <span class="gen-label">前景</span>
              <input type="color" value={fg} onInput={(e) => setFg((e.target as HTMLInputElement).value)} />
              <span class="gen-label">背景</span>
              <input type="color" value={bg} onInput={(e) => setBg((e.target as HTMLInputElement).value)} />
            </div>
          </div>
        </div>

        <div class="viewer-section gen-preview-section" ref={previewRef}>
          {err ? (
            <div class="decoder-error">{err}</div>
          ) : preview ? (
            <img
              class="gen-preview"
              src={`data:image/png;base64,${preview}`}
              width={size}
              height={size}
              alt="二维码预览"
            />
          ) : (
            <div class="gen-placeholder">预览将显示在这里</div>
          )}
        </div>
      </div>

      <div class="gen-footer">
        <span class="gen-status">{status}</span>
        <button class="viewer-btn" onClick={handleCopy} disabled={!preview}>
          <FluentIcon name="copy" size={14} /> 复制
        </button>
        <button class="viewer-btn" onClick={handleSave} disabled={!preview}>保存</button>
      </div>
    </div>
  );
}
