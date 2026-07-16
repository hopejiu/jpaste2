import { useEffect, useMemo, useRef, useState } from 'preact/hooks';
import { FluentIcon } from '../../components/fluent-icon';
import { copyToClipboard } from '../../lib/clipboard';
import { api } from '../../lib/invoke';
import { useViewerEntry } from '../../hooks/use-viewer-entry';
import { useJsonEditor } from '../../hooks/use-json-editor';
import { genCode, CODEGEN_LANGS } from '../../lib/utils/curl-codegen';
import type { CodegenLang } from '../../lib/utils/curl-codegen';
import { info as logInfo, error as logError } from '../../lib/logger';

const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'];
const HTTP_SCHEMES = ['http', 'https'];
const RAW_TRUNCATE = 200_000; // chars; above this the raw <pre> is truncated to avoid freezing

interface KVRow {
  key: string;
  value: string;
  enabled: boolean; // ponytail: unchecked rows are skipped from the outgoing request for quick debugging
}

// ── Lightweight curl parser (no WASM, no Node deps) ────────────────────

interface ParsedCurl {
  method: string;
  url: string;
  headers: Record<string, string>;
  body: string;
}

function parseCurl(cmd: string): ParsedCurl {
  const result: ParsedCurl = { method: 'GET', url: '', headers: {}, body: '' };
  const tokens = tokenize(cmd);
  let i = 0;

  if (tokens[i]?.toLowerCase() === 'curl') i++;

  while (i < tokens.length) {
    const tok = tokens[i];

    if (tok === '-X' || tok === '--request') {
      i++;
      if (i < tokens.length) result.method = tokens[i].toUpperCase();
    } else if (tok === '-H' || tok === '--header') {
      i++;
      if (i < tokens.length) {
        const hdr = tokens[i];
        const colon = hdr.indexOf(':');
        if (colon > 0) {
          result.headers[hdr.substring(0, colon).trim()] = hdr.substring(colon + 1).trim();
        }
      }
    } else if (tok === '-d' || tok === '--data' || tok === '--data-raw') {
      i++;
      if (i < tokens.length) result.body = tokens[i];
    } else if (tok.startsWith('http://') || tok.startsWith('https://')) {
      result.url = tok;
    } else if (!tok.startsWith('-') && !result.url) {
      result.url = tok;
    }
    i++;
  }

  // Auto-detect POST if body present
  if (result.body && result.method === 'GET') result.method = 'POST';

  return result;
}

function tokenize(input: string): string[] {
  const tokens: string[] = [];
  let current = '';
  let quote: string | null = null;

  for (const ch of input) {
    if (quote) {
      if (ch === quote) quote = null;
      else current += ch;
    } else if (ch === '"' || ch === "'") {
      quote = ch;
    } else if (ch === ' ' || ch === '\t') {
      if (current) { tokens.push(current); current = ''; }
    } else {
      current += ch;
    }
  }
  if (current) tokens.push(current);
  return tokens;
}

// ── URL helpers ─────────────────────────────────────────────────────────

function stripHttpProtocol(value: string): { scheme: string; host: string } | null {
  for (const s of HTTP_SCHEMES) {
    const prefix = s + '://';
    if (value.startsWith(prefix)) return { scheme: s, host: value.slice(prefix.length) };
  }
  return null;
}

function parseUrl(fullUrl: string) {
  try {
    const u = new URL(fullUrl);
    const params: KVRow[] = [];
    for (const [k, v] of u.searchParams.entries()) params.push({ key: k, value: v, enabled: true });
    return { base: u.origin + u.pathname, params };
  } catch {
    return { base: fullUrl, params: [] as KVRow[] };
  }
}

function buildUrl(host: string, scheme: string, params: KVRow[]) {
  const filtered = params.filter(p => p.key.trim() && p.enabled);
  const base = `${scheme}://${host}`;
  if (filtered.length === 0) return base;
  const qs = filtered.map(p => encodeURIComponent(p.key.trim()) + '=' + encodeURIComponent(p.value)).join('&');
  return base + '?' + qs;
}

function statusClass(code: number) {
  if (code >= 200 && code < 300) return 'ok';
  if (code >= 300 && code < 400) return 'redirect';
  if (code >= 400) return 'error';
  return '';
}

function getHeader(headers: [string, string][], name: string): string {
  const lower = name.toLowerCase();
  return headers.find(([k]) => k.toLowerCase() === lower)?.[1] ?? '';
}

type BodyParse =
  | { state: 'none' }
  | { state: 'empty' }
  | { state: 'json'; data: unknown }
  | { state: 'invalid' };

// ── Section Header (collapsible card) ───────────────────────────────────

function SectionHeader({ title, desc, defaultOpen = true, children }: { title: string; desc?: string; defaultOpen?: boolean; children: any }) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div class="curl-card">
      <div class="curl-card-header" onClick={() => setOpen(!open)}>
        <span class="curl-card-arrow"><FluentIcon name={open ? 'chevronDown' : 'chevronUp'} size={12} /></span>
        <span class="curl-card-title">{title}</span>
        {desc && <span class="curl-card-desc">{desc}</span>}
      </div>
      {open && children}
    </div>
  );
}

// ── KV Table ─────────────────────────────────────────────────────────────

function KVTable({ rows, onChange, keyPlaceholder, valuePlaceholder }: {
  rows: KVRow[];
  onChange: (rows: KVRow[]) => void;
  keyPlaceholder: string;
  valuePlaceholder: string;
}) {
  const ensureLast = (list: KVRow[]) => {
    if (list.length === 0 || list[list.length - 1].key !== '' || list[list.length - 1].value !== '') {
      return [...list, { key: '', value: '', enabled: true }];
    }
    return list;
  };

  const updateRow = (idx: number, field: 'key' | 'value', val: string) => {
    const next = rows.map((r, i) => i === idx ? { ...r, [field]: val } : r);
    onChange(ensureLast(next));
  };

  const updateEnabled = (idx: number, val: boolean) => {
    onChange(rows.map((r, i) => i === idx ? { ...r, enabled: val } : r));
  };

  const removeRow = (idx: number) => {
    onChange(rows.filter((_, i) => i !== idx));
  };

  const addRow = () => {
    onChange([...rows, { key: '', value: '', enabled: true }]);
  };

  return (
    <div class="kv-table">
      {rows.map((row, idx) => (
        <div class="kv-row" key={idx}>
          <input class="kv-check" type="checkbox" checked={row.enabled} onChange={(e) => updateEnabled(idx, (e.target as HTMLInputElement).checked)} title="启用/禁用此行" />
          <input class="kv-key" value={row.key} onInput={(e) => updateRow(idx, 'key', (e.target as HTMLInputElement).value)} placeholder={keyPlaceholder} />
          <span class="kv-colon">:</span>
          <input class="kv-value" value={row.value} onInput={(e) => updateRow(idx, 'value', (e.target as HTMLInputElement).value)} placeholder={valuePlaceholder} />
          <button class="kv-remove" onClick={() => removeRow(idx)} title="删除此行" aria-label="删除此行"><FluentIcon name="close" size={14} /></button>
        </div>
      ))}
      <button class="kv-add-btn" onClick={addRow} title="添加一行">
        <FluentIcon name="add" size={13} /> 添加
      </button>
    </div>
  );
}

// ── Main Component ──────────────────────────────────────────────────────

export function CurlViewPage() {
  const { entryId, content, loading, error: fetchError } = useViewerEntry();
  const [parseError, setParseError] = useState('');
  const { containerRef, updateJson, destroyEditor } = useJsonEditor();

  const [method, setMethod] = useState('GET');
  const [urlBase, setUrlBase] = useState('');
  const [httpScheme, setHttpScheme] = useState('https');
  const [queryParams, setQueryParams] = useState<KVRow[]>([]);
  const [headers, setHeaders] = useState<KVRow[]>([]);
  const [body, setBody] = useState('');
  const [followRedirects, setFollowRedirects] = useState(false);
  const [timeout, setTimeout_] = useState(30);

  const [sendLoading, setSendLoading] = useState(false);
  const [response, setResponse] = useState<{
    status_code: number;
    status_text: string;
    headers: [string, string][];
    body: string;
    duration_ms: number;
  } | null>(null);

  const [respCollapsed, setRespCollapsed] = useState(false);
  const [respHeight, setRespHeight] = useState<number | null>(null); // ponytail: null = flex-fill; drag sets explicit px
  const respPanelRef = useRef<HTMLDivElement>(null);

  // Drag the divider above the response panel to resize it.
  const startRespResize = (e: MouseEvent) => {
    e.preventDefault();
    const panel = respPanelRef.current;
    if (!panel) return;
    const startY = e.clientY;
    const startH = panel.getBoundingClientRect().height;
    const onMove = (ev: MouseEvent) => {
      const next = Math.max(120, Math.min(window.innerHeight * 0.85, startH + (startY - ev.clientY)));
      setRespHeight(next);
    };
    const onUp = () => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      document.body.style.userSelect = '';
    };
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
    document.body.style.userSelect = 'none';
  };

  // Response body view mode: auto-detect / force JSON / force raw
  const [bodyView, setBodyView] = useState<'auto' | 'json' | 'raw'>('auto');
  const [showFullBody, setShowFullBody] = useState(false);

  // Codegen
  const [codeLang, setCodeLang] = useState<CodegenLang>('python');

  useEffect(() => {
    if (entryId <= 0) return; // blank/toolbox viewer: hook already cleared loading/error
    if (!content) { setParseError('条目内容为空'); return; }
    try {
      const parsed = parseCurl(content);
      logInfo('CurlViewPage: parsed OK', { method: parsed.method, url: parsed.url });
      setParseError('');
      applyParsed(parsed);
    } catch (e: any) {
      logError('CurlViewPage: parse error', e);
      setParseError('curl 解析失败: ' + (e?.message || e));
    }
  }, [content]);

  function applyParsed(parsed: ParsedCurl) {
    setMethod(parsed.method || 'GET');
    const rawUrl = parsed.url || '';
    const { base, params } = parseUrl(rawUrl);
    const detected = stripHttpProtocol(base);
    if (detected) {
      setHttpScheme(detected.scheme);
      setUrlBase(detected.host);
    } else {
      setUrlBase(base);
    }
    setQueryParams(params.length > 0 ? params : [{ key: '', value: '', enabled: true }]);
    const hdrRows = Object.entries(parsed.headers).map(([key, value]) => ({ key, value, enabled: true }));
    setHeaders(hdrRows.length > 0 ? hdrRows : [{ key: '', value: '', enabled: true }]);
    setBody(parsed.body || '');
  }

  const fullUrl = buildUrl(urlBase, httpScheme, queryParams);

  const handleSend = async () => {
    const url = fullUrl;
    if (!url) return;

    setSendLoading(true);
    setResponse(null);
    setShowFullBody(false);
    setRespCollapsed(true); // 请求前自动收起上一次的响应
    const start = Date.now();

    try {
      const result = await api.sendCurlRequest({
        method,
        url,
        headers: Object.fromEntries(headers.filter(h => h.enabled && h.key.trim()).map(h => [h.key.trim(), h.value])),
        body,
        followRedirects,
        timeout,
      });
      setResponse({ ...result, duration_ms: Date.now() - start });
    } catch (e: any) {
      setResponse({
        status_code: 0,
        status_text: 'Request Failed',
        headers: [],
        body: String(e?.message || e),
        duration_ms: Date.now() - start,
      });
    }
    setSendLoading(false);
    setRespCollapsed(false); // 请求后展开
  };

  // ── Response body parsing / view resolution ───────────────────────────
  const bodyParse = useMemo<BodyParse>(() => {
    if (!response) return { state: 'none' };
    if (response.body.trim() === '') return { state: 'empty' };
    try { return { state: 'json', data: JSON.parse(response.body) }; }
    catch { return { state: 'invalid' }; }
  }, [response]);

  const ctJson = useMemo(() => {
    if (!response) return false;
    const ct = getHeader(response.headers, 'content-type');
    return /application\/json|[\/+]json/i.test(ct);
  }, [response]);

  const resolved: 'json' | 'raw' = useMemo(() => {
    if (bodyView === 'json') return bodyParse.state === 'json' ? 'json' : 'raw';
    if (bodyView === 'raw') return 'raw';
    // auto
    if (ctJson || bodyParse.state === 'json') return 'json';
    return 'raw';
  }, [bodyView, bodyParse, ctJson]);

  const jsonNote = resolved === 'raw' && bodyParse.state !== 'json' && (bodyView === 'json' || (bodyView === 'auto' && ctJson));

  useEffect(() => {
    if (resolved === 'json' && bodyParse.state === 'json') {
      updateJson(bodyParse.data).catch((e) => logError('CurlViewPage: json update', e));
    } else {
      destroyEditor();
    }
  }, [resolved, bodyParse]);

  useEffect(() => () => destroyEditor(), []);

  // ── Request-side helpers ──────────────────────────────────────────────
  const formatRequestBody = () => {
    if (!body.trim()) return;
    try {
      setBody(JSON.stringify(JSON.parse(body), null, 2));
    } catch { /* not JSON, leave as-is */ }
  };

  const addBearer = () => {
    if (headers.some(h => h.key.trim().toLowerCase() === 'authorization')) return;
    setHeaders([...headers, { key: 'Authorization', value: 'Bearer ', enabled: true }]);
  };

  // ── Codegen ───────────────────────────────────────────────────────────
  const codeSnippet = useMemo(
    () => genCode(codeLang, method, fullUrl, headers.filter(h => h.enabled && h.key.trim()), body),
    [codeLang, method, fullUrl, headers, body],
  );

  const error = fetchError || parseError;
  if (loading) return <div class="viewer-loading">加载中...</div>;
  if (error) return <div class="viewer-error">{error}</div>;

  const handleCopyCurl = () => {
    const hdrParts = headers.filter(h => h.enabled && h.key.trim()).map(h => `-H '${h.key.trim()}: ${h.value}'`);
    const bodyParts = body ? [`-d '${body}'`] : [];
    const methodPart = method !== 'GET' ? `-X ${method}` : '';
    const parts = ['curl', methodPart, `'${fullUrl}'`, ...hdrParts, ...bodyParts].filter(Boolean);
    copyToClipboard(parts.join(' \\\n  '));
  };

  // ── Response header rendering ─────────────────────────────────────────
  const cookieEntries = response ? response.headers.filter(([k]) => k.toLowerCase() === 'set-cookie') : [];
  const otherHeaders = response
    ? response.headers.filter(([k]) => k.toLowerCase() !== 'set-cookie')
        .slice().sort((a, b) => a[0].toLowerCase().localeCompare(b[0].toLowerCase()))
    : [];

  const rawTruncated = !!response && resolved === 'raw' && !showFullBody && response.body.length > RAW_TRUNCATE;
  const rawDisplay = rawTruncated && response ? response.body.slice(0, RAW_TRUNCATE) : (response?.body ?? '');

  const copyValue = (val: string) => copyToClipboard(val);

  return (
    <div class="viewer-page">
      <div class="viewer-toolbar" data-tauri-drag-region>
        <div class="viewer-toolbar-icon">
          <FluentIcon name="terminal" size={20} />
        </div>
        <span class="viewer-title">HTTP 调试</span>
        <button class="viewer-btn" onClick={handleCopyCurl} title="复制 curl 命令">
          <FluentIcon name="copy" size={14} /> 复制 curl
        </button>
      </div>

      <div class="curl-request-panel curl-request-panel--with-response">
        <SectionHeader title="请求" desc={`${method} ${fullUrl.substring(0, 60)}${fullUrl.length > 60 ? '…' : ''}`}>
          <div class="curl-card-body">
            <div class="curl-url-row">
              <select class="curl-method-select" data-method={method} value={method} onChange={(e) => setMethod((e.target as HTMLSelectElement).value)}>
                {METHODS.map(m => <option key={m} value={m}>{m}</option>)}
              </select>
              <select class="curl-scheme-select" value={httpScheme} onChange={(e) => setHttpScheme((e.target as HTMLSelectElement).value)}>
                {HTTP_SCHEMES.map(s => <option key={s} value={s}>{s}://</option>)}
              </select>
              <span class="curl-scheme-sep">://</span>
              <input class="curl-url-input" value={urlBase} onInput={(e) => setUrlBase((e.target as HTMLInputElement).value)} placeholder="example.com/api/path" />
              <button class="viewer-btn primary" onClick={handleSend} disabled={sendLoading || !urlBase.trim()}>
                {sendLoading ? '发送中...' : '发送'}
              </button>
            </div>
          </div>
        </SectionHeader>

        <SectionHeader title="查询参数" desc={`${queryParams.filter(p => p.enabled && p.key.trim()).length} 个启用`}>
          <div class="curl-card-body">
            <KVTable rows={queryParams} onChange={setQueryParams} keyPlaceholder="键" valuePlaceholder="值" />
          </div>
        </SectionHeader>

        <SectionHeader title="请求头" desc={`${headers.filter(h => h.enabled && h.key.trim()).length} 个启用`}>
          <div class="curl-card-body">
            <div class="curl-req-head-actions">
              <button class="viewer-btn sm" onClick={addBearer} title="添加 Bearer 认证头">
                <FluentIcon name="key" size={13} /> Bearer
              </button>
            </div>
            <KVTable rows={headers} onChange={setHeaders} keyPlaceholder="Header-Name" valuePlaceholder="Header-Value" />
          </div>
        </SectionHeader>

        <SectionHeader title="请求体" defaultOpen={!!body}>
          <div class="curl-card-body">
            <div class="curl-req-head-actions">
              <button class="viewer-btn sm" onClick={formatRequestBody} title="格式化 JSON 请求体" disabled={!body.trim()}>
                <FluentIcon name="code" size={13} /> 格式化
              </button>
            </div>
            <textarea class="curl-body-input" value={body} onInput={(e) => setBody((e.target as HTMLTextAreaElement).value)} rows={4} placeholder="请求体内容..." />
          </div>
        </SectionHeader>

        <SectionHeader title="选项">
          <div class="curl-options-row">
            <label class="curl-opt-label">
              <input type="checkbox" checked={followRedirects} onChange={(e) => setFollowRedirects((e.target as HTMLInputElement).checked)} /> 跟随重定向
            </label>
            <label class="curl-opt-label">
              超时 <input class="curl-timeout-input" type="number" value={timeout} onInput={(e) => setTimeout_(parseInt((e.target as HTMLInputElement).value) || 30)} min="1" max="120" /> 秒
            </label>
          </div>
        </SectionHeader>

        <SectionHeader title="代码生成">
          <div class="curl-card-body">
            <div class="curl-codegen-row">
              <select class="curl-lang-select" value={codeLang} onChange={(e) => setCodeLang((e.target as HTMLSelectElement).value as CodegenLang)}>
                {CODEGEN_LANGS.map(l => <option key={l} value={l}>{l}</option>)}
              </select>
              <button class="viewer-btn sm" onClick={() => copyToClipboard(codeSnippet)} title="复制代码片段">
                <FluentIcon name="copy" size={13} /> 复制
              </button>
            </div>
            <pre class="curl-codegen-output">{codeSnippet}</pre>
          </div>
        </SectionHeader>
      </div>

      {!response && (
        <>
          <div class="curl-resize-handle" onMouseDown={respCollapsed ? undefined : startRespResize} title="拖动调节响应区高度" role="separator" aria-orientation="horizontal">
            <button
              class="curl-resize-toggle"
              onMouseDown={(e) => e.stopPropagation()}
              onClick={() => setRespCollapsed(!respCollapsed)}
              title={respCollapsed ? '展开响应' : '折叠响应'}
            >
              <FluentIcon name={respCollapsed ? 'chevronUp' : 'chevronDown'} size={14} />
            </button>
          </div>
          <div class="curl-response-panel collapsed">
            <div class="curl-response-statusbar" style="cursor: pointer" onClick={() => setRespCollapsed(!respCollapsed)}>
              <span class="curl-status">未发送</span>
              {!respCollapsed && (
                <span style={{ fontSize: 'var(--font-size-sm)', color: 'var(--color-text-muted)' }}>配置请求参数后点击"发送"</span>
              )}
            </div>
          </div>
        </>
      )}

      {response && (
        <>
          <div class="curl-resize-handle" onMouseDown={respCollapsed ? undefined : startRespResize} title="拖动调节响应区高度" role="separator" aria-orientation="horizontal">
            <button
              class="curl-resize-toggle"
              onMouseDown={(e) => e.stopPropagation()}
              onClick={() => setRespCollapsed(!respCollapsed)}
              title={respCollapsed ? '展开响应' : '折叠响应'}
            >
              <FluentIcon name={respCollapsed ? 'chevronUp' : 'chevronDown'} size={14} />
            </button>
          </div>
          <div
            ref={respPanelRef}
            class={`curl-response-panel ${respCollapsed ? 'collapsed' : ''}`}
            style={respHeight != null && !respCollapsed ? { flex: `0 0 ${respHeight}px` } : undefined}
          >
          <div class="curl-response-statusbar" style="cursor: pointer" onClick={() => setRespCollapsed(!respCollapsed)}>
            <span class={`curl-status ${statusClass(response.status_code)}`}>
              {response.status_code} {response.status_text}
            </span>
            <span class="curl-duration">{response.duration_ms}ms</span>
            <span class="curl-body-size">{response.body.length} bytes</span>
            <div class="curl-response-actions">
              <button class="viewer-btn" onClick={(e) => { e.stopPropagation(); copyToClipboard(response.body); }}>
                <FluentIcon name="copy" size={14} /> 复制响应
              </button>
            </div>
          </div>
          {!respCollapsed && (
            <div class="curl-response-content">
              {otherHeaders.length > 0 && (
                <div class="curl-resp-section">
                  <div class="curl-resp-section-head">
                    <span class="curl-resp-section-title">响应头 <span class="curl-resp-count">({otherHeaders.length})</span></span>
                  </div>
                  <div class="curl-resp-headers">
                    {otherHeaders.map(([k, v], idx) => (
                      <div class="curl-resp-hdr-row" key={idx}>
                        <span class="curl-resp-hdr-key">{k}:</span>{' '}
                        <span class="curl-resp-hdr-value curl-copyable" title="点击复制值" onClick={() => copyValue(v)}>{v}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {cookieEntries.length > 0 && (
                <div class="curl-resp-section">
                  <div class="curl-resp-section-head">
                    <span class="curl-resp-section-title">Cookie <span class="curl-resp-count">({cookieEntries.length})</span></span>
                  </div>
                  <div class="curl-cookie-block">
                    {cookieEntries.map(([, v], idx) => {
                      const split = v.indexOf(';');
                      const nv = (split >= 0 ? v.slice(0, split) : v).trim();
                      const attrs = split >= 0 ? v.slice(split + 1).trim() : '';
                      return (
                        <div class="curl-cookie-item" key={idx}>
                          <span class="curl-cookie-name" title="点击复制" onClick={() => copyValue(nv)}>{nv}</span>
                          {attrs && <span class="curl-cookie-attr">{attrs}</span>}
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}

              <div class="curl-resp-section">
                <div class="curl-resp-section-head">
                  <span class="curl-resp-section-title">响应体</span>
                  <div class="curl-body-view-controls">
                    <button class={`curl-seg ${bodyView === 'auto' ? 'active' : ''}`} onClick={() => setBodyView('auto')}>自动</button>
                    <button class={`curl-seg ${bodyView === 'json' ? 'active' : ''}`} onClick={() => setBodyView('json')}>JSON</button>
                    <button class={`curl-seg ${bodyView === 'raw' ? 'active' : ''}`} onClick={() => setBodyView('raw')}>原样</button>
                  </div>
                </div>
                {resolved === 'json' && bodyParse.state === 'json' ? (
                  <div ref={containerRef} class="curl-json-container" style={{ height: '360px' }} />
                ) : (
                  <>
                    <pre class="curl-resp-body">{rawDisplay}</pre>
                    {rawTruncated && (
                      <button class="viewer-btn sm" onClick={() => setShowFullBody(true)}>
                        加载全部 ({Math.round(response.body.length / 1024)} KB)
                      </button>
                    )}
                    {jsonNote && <div class="curl-json-note">JSON 解析失败，已按原样显示</div>}
                  </>
                )}
              </div>
            </div>
          )}
        </div>
      </>)}
    </div>
  );
}
