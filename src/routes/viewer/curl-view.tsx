import { useEffect, useState } from 'preact/hooks';
import { FluentIcon } from '../../components/fluent-icon';
import { api } from '../../lib/invoke';
import { copyToClipboard } from '../../lib/clipboard';
import { useEntryId } from '../../hooks/use-entry-id';
import { info as logInfo, error as logError } from '../../lib/logger';

const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'];
const HTTP_SCHEMES = ['http', 'https'];

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
    const params: { key: string; value: string }[] = [];
    for (const [k, v] of u.searchParams.entries()) params.push({ key: k, value: v });
    return { base: u.origin + u.pathname, params };
  } catch {
    return { base: fullUrl, params: [] };
  }
}

function buildUrl(host: string, scheme: string, params: { key: string; value: string }[]) {
  const filtered = params.filter(p => p.key.trim());
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
  rows: { key: string; value: string }[];
  onChange: (rows: { key: string; value: string }[]) => void;
  keyPlaceholder: string;
  valuePlaceholder: string;
}) {
  const ensureLast = (list: { key: string; value: string }[]) => {
    if (list.length === 0 || list[list.length - 1].key !== '' || list[list.length - 1].value !== '') {
      return [...list, { key: '', value: '' }];
    }
    return list;
  };

  const updateRow = (idx: number, field: 'key' | 'value', val: string) => {
    const next = rows.map((r, i) => i === idx ? { ...r, [field]: val } : r);
    onChange(ensureLast(next));
  };

  const removeRow = (idx: number) => {
    if (rows.length <= 1) return;
    onChange(rows.filter((_, i) => i !== idx));
  };

  return (
    <div class="kv-table">
      {rows.map((row, idx) => (
        <div class="kv-row" key={idx}>
          <input class="kv-key" value={row.key} onInput={(e) => updateRow(idx, 'key', (e.target as HTMLInputElement).value)} placeholder={keyPlaceholder} />
          <span class="kv-colon">:</span>
          <input class="kv-value" value={row.value} onInput={(e) => updateRow(idx, 'value', (e.target as HTMLInputElement).value)} placeholder={valuePlaceholder} />
          <button class="kv-remove" onClick={() => removeRow(idx)} title="删除此行" aria-label="删除此行"><FluentIcon name="close" size={14} /></button>
        </div>
      ))}
    </div>
  );
}

// ── Main Component ──────────────────────────────────────────────────────

export function CurlViewPage() {
  const entryId = useEntryId();

  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const [method, setMethod] = useState('GET');
  const [urlBase, setUrlBase] = useState('');
  const [httpScheme, setHttpScheme] = useState('https');
  const [queryParams, setQueryParams] = useState<{ key: string; value: string }[]>([]);
  const [headers, setHeaders] = useState<{ key: string; value: string }[]>([]);
  const [body, setBody] = useState('');
  const [followRedirects, setFollowRedirects] = useState(false);
  const [timeout, setTimeout_] = useState(30);

  const [sendLoading, setSendLoading] = useState(false);
  const [response, setResponse] = useState<{
    status_code: number;
    status_text: string;
    headers: Record<string, string>;
    body: string;
    duration_ms: number;
  } | null>(null);

  const [respCollapsed, setRespCollapsed] = useState(false);

  useEffect(() => {
    if (!entryId) {
      setLoading(false);
      setError('无效的条目 ID');
      return;
    }

    logInfo('CurlViewPage: mounted', { entryId });
    api.getEntryContent(entryId)
      .then(async (data) => {
        logInfo('CurlViewPage: content fetched', { dataLen: data?.length });
        if (!data) {
          setError('条目内容为空');
          setLoading(false);
          return;
        }
        try {
          const parsed = parseCurl(data);
          logInfo('CurlViewPage: parsed OK', { method: parsed.method, url: parsed.url });
          applyParsed(parsed);
        } catch (e: any) {
          logError('CurlViewPage: parse error', e);
          setError('curl 解析失败: ' + (e?.message || e));
        }
        setLoading(false);
      })
      .catch((e) => {
        logError('CurlViewPage: fetch error', e);
        setError(String(e?.message || '获取数据失败'));
        setLoading(false);
      });
  }, [entryId]);

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
    setQueryParams(params.length > 0 ? params : [{ key: '', value: '' }]);
    const hdrRows = Object.entries(parsed.headers).map(([key, value]) => ({ key, value }));
    setHeaders(hdrRows.length > 0 ? hdrRows : [{ key: '', value: '' }]);
    setBody(parsed.body || '');
  }

  const handleSend = async () => {
    const url = buildUrl(urlBase, httpScheme, queryParams);
    if (!url) return;

    setSendLoading(true);
    setResponse(null);
    const start = Date.now();

    try {
      const result = await api.sendCurlRequest({
        method,
        url,
        headers: Object.fromEntries(headers.filter(h => h.key.trim()).map(h => [h.key.trim(), h.value])),
        body,
        followRedirects,
        timeout,
      });
      setResponse({ ...result, duration_ms: Date.now() - start });
    } catch (e: any) {
      setResponse({
        status_code: 0,
        status_text: 'Request Failed',
        headers: {},
        body: String(e?.message || e),
        duration_ms: Date.now() - start,
      });
    }
    setSendLoading(false);
  };

  if (loading) return <div class="viewer-loading">加载中...</div>;
  if (error) return <div class="viewer-error">{error}</div>;

  const fullUrl = buildUrl(urlBase, httpScheme, queryParams);

  const handleCopyCurl = () => {
    const hdrParts = headers.filter(h => h.key.trim()).map(h => `-H '${h.key.trim()}: ${h.value}'`);
    const bodyParts = body ? [`-d '${body}'`] : [];
    const methodPart = method !== 'GET' ? `-X ${method}` : '';
    const parts = ['curl', methodPart, `'${fullUrl}'`, ...hdrParts, ...bodyParts].filter(Boolean);
    copyToClipboard(parts.join(' \\\n  '));
  };

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
        <SectionHeader title="请求" desc={`${method} ${fullUrl.substring(0, 60)}...`}>
          <div class="curl-card-body">
            <div class="curl-url-row">
              <select class="curl-method-select" value={method} onChange={(e) => setMethod((e.target as HTMLSelectElement).value)}>
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

        <SectionHeader title="查询参数" desc={`${queryParams.filter(p => p.key.trim()).length} 个参数`}>
          <div class="curl-card-body">
            <KVTable rows={queryParams} onChange={setQueryParams} keyPlaceholder="键" valuePlaceholder="值" />
          </div>
        </SectionHeader>

        <SectionHeader title="请求头" desc={`${headers.filter(h => h.key.trim()).length} 个头`}>
          <div class="curl-card-body">
            <KVTable rows={headers} onChange={setHeaders} keyPlaceholder="Header-Name" valuePlaceholder="Header-Value" />
          </div>
        </SectionHeader>

        <SectionHeader title="请求体" defaultOpen={!!body}>
          <div class="curl-card-body">
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
      </div>

      {!response && (
        <div class="curl-empty-state">
          <FluentIcon name="terminal" size={48} style={{ color: 'var(--color-text-muted)', marginBottom: '16px' }} />
          <div style={{ fontSize: '14px', color: 'var(--color-text-secondary)' }}>配置请求参数后点击"发送"</div>
          <div style={{ fontSize: '12px', color: 'var(--color-text-muted)', marginTop: '4px' }}>响应将显示在这里</div>
        </div>
      )}

      {response && (
        <div class={`curl-response-panel ${respCollapsed ? 'collapsed' : ''}`}>
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
              {Object.keys(response.headers).length > 0 && (
                <div class="curl-resp-section">
                  <div class="curl-resp-section-title">响应头 <span class="curl-resp-count">({Object.keys(response.headers).length})</span></div>
                  <div class="curl-resp-headers">
                    {Object.entries(response.headers).map(([k, v]) => (
                      <div key={k}><span class="curl-resp-hdr-key">{k}:</span> <span class="curl-resp-hdr-value">{v}</span></div>
                    ))}
                  </div>
                </div>
              )}
              <div class="curl-resp-section">
                <div class="curl-resp-section-title">响应体</div>
                <pre class="curl-resp-body">{response.body}</pre>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
