import { useState, useEffect, useRef, useCallback } from 'preact/hooks';
import { FluentIcon } from '../../components/fluent-icon';
import { api } from '../../lib/invoke';
import { error as logError, setComponent } from '../../lib/logger';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type { ShareItem, ShareUrl } from '../../lib/types';

setComponent('share');

function humanSize(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let s = bytes;
  let i = 0;
  while (s >= 1024 && i < units.length - 1) {
    s /= 1024;
    i++;
  }
  return i === 0 ? `${bytes} ${units[0]}` : `${s.toFixed(1)} ${units[i]}`;
}

export function SharePage() {
  const [urls, setUrls] = useState<ShareUrl[]>([]);
  const [items, setItems] = useState<ShareItem[]>([]);
  const [text, setText] = useState('');
  const [dragging, setDragging] = useState(false);
  const [status, setStatus] = useState('');
  const [qrShown, setQrShown] = useState<Record<string, string>>({});
  const [urlsExpanded, setUrlsExpanded] = useState(true);
  const statusTimer = useRef<number | undefined>(undefined);

  const flash = useCallback((msg: string) => {
    setStatus(msg);
    if (statusTimer.current) clearTimeout(statusTimer.current);
    statusTimer.current = window.setTimeout(() => setStatus(''), 1800);
  }, []);

  const refreshItems = useCallback(() => {
    api.listShareItems().then(setItems).catch((e) => logError('list items', e));
  }, []);

  // Start server on mount; stop is automatic via window Destroyed event.
  useEffect(() => {
    api.startShareServer()
      .then((u) => setUrls(u))
      .catch((e) => flash(`启动失败: ${String(e?.message || e)}`));
    refreshItems();

    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === 'enter' || p.type === 'over') {
          setDragging(true);
        } else if (p.type === 'leave') {
          setDragging(false);
        } else if (p.type === 'drop') {
          setDragging(false);
          p.paths.forEach((path) => addFile(path));
        }
      })
      .then((fn) => { unlisten = fn; })
      .catch((e) => logError('drag-drop', e));

    return () => { if (unlisten) unlisten(); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const addFile = useCallback((path: string) => {
    api.addShareFile(path)
      .then(() => { refreshItems(); flash('已添加'); })
      .catch((e) => flash(`添加失败: ${String(e?.message || e)}`));
  }, [refreshItems, flash]);

  const handlePick = () => {
    api.pickShareFiles()
      .then((paths) => paths.forEach(addFile))
      .catch((e) => logError('pick files', e));
  };

  const handleAddText = () => {
    const t = text.trim();
    if (!t) { flash('文本不能为空'); return; }
    api.addShareText('', t)
      .then(() => { setText(''); refreshItems(); flash('已添加文本'); })
      .catch((e) => flash(`添加失败: ${String(e?.message || e)}`));
  };

  const handleRemove = (id: string) => {
    api.removeShareItem(id)
      .then(refreshItems)
      .catch((e) => logError('remove', e));
  };

  const copyUrl = (url: string) => {
    navigator.clipboard.writeText(url)
      .then(() => flash('已复制链接'))
      .catch(() => flash('复制失败'));
  };

  const toggleQr = (url: string) => {
    setQrShown((prev) => {
      const next = { ...prev };
      if (next[url]) {
        delete next[url];
      } else {
        api.generateQr({ content: url, size: 220, ecLevel: 'M', margin: 4, fg: '#000000', bg: '#ffffff' })
          .then((b64) => setQrShown((p) => ({ ...p, [url]: b64 })))
          .catch((e) => logError('qr', e));
      }
      return next;
    });
  };

  return (
    <div class="viewer-page">
      <div class="viewer-toolbar" data-tauri-drag-region>
        <div class="viewer-toolbar-icon">
          <FluentIcon name="globe" size={20} />
        </div>
        <span class="viewer-title">HTTP 共享</span>
      </div>

      <div class="viewer-content">
        <div class="share-risk">
          <FluentIcon name="warning" size={16} />
          <span>局域网内任何设备均可访问此链接，公共 WiFi 下请谨慎使用。</span>
        </div>

        {/* Access URLs */}
        <div class="viewer-section">
          <button
            class="viewer-section-head"
            onClick={() => setUrlsExpanded((v) => !v)}
            title={urlsExpanded ? '收起' : '展开'}
          >
            <FluentIcon name="chevronRight" size={14} className={`chevron ${urlsExpanded ? 'open' : ''}`} />
            <span class="viewer-section-title">访问地址</span>
            <span class="viewer-section-count">{urls.length}</span>
          </button>
          {urlsExpanded && (
            <>
              <div class="viewer-section-desc">将任意一条链接发给同一局域网的设备即可访问。</div>
              {urls.length === 0 ? (
                <div class="share-empty">未获取到可用网络地址（请检查网卡）。</div>
              ) : (
                <div class="share-urls">
                  {urls.map((u) => (
                    <div class="share-url-row">
                      <span class="share-iface" title={u.ip}>{u.name}</span>
                      <code class="share-url">{u.url}</code>
                      <button class="viewer-btn sm" title="复制链接" onClick={() => copyUrl(u.url)}>
                        <FluentIcon name="copy" size={13} /> 复制
                      </button>
                      <button class="viewer-btn sm" title="二维码" onClick={() => toggleQr(u.url)}>
                        <FluentIcon name="qrCode" size={13} /> 二维码
                      </button>
                      {qrShown[u.url] && (
                        <div class="share-qr">
                          <img src={`data:image/png;base64,${qrShown[u.url]}`} alt="二维码" />
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </>
          )}
        </div>

        {/* Add content */}
        <div class="viewer-section">
          <div class="viewer-section-title">添加共享内容</div>
          <div class={`share-drop ${dragging ? 'over' : ''}`}>
            <FluentIcon name="cloudUpload" size={26} />
            <span>拖拽文件到此处</span>
          </div>
          <div class="share-add-actions">
            <button class="viewer-btn primary" onClick={handlePick}>
              <FluentIcon name="folderOpen" size={14} /> 选择文件
            </button>
          </div>
          <div class="share-text-wrap">
            <textarea
              class="share-text"
              value={text}
              onInput={(e) => setText((e.target as HTMLTextAreaElement).value)}
              placeholder="或在此粘贴文本，点击添加…"
              rows={3}
            />
            <button class="viewer-btn" onClick={handleAddText} disabled={!text.trim()}>
              添加文本
            </button>
          </div>
        </div>

        {/* Items */}
        <div class="viewer-section">
          <div class="viewer-section-title">共享条目（{items.length}）</div>
          {items.length === 0 ? (
            <div class="share-empty">还没有共享内容。</div>
          ) : (
            <div class="share-items">
              {items.map((it) => (
                <div class="share-item">
                  <FluentIcon name={it.kind === 'file' ? 'document' : 'type'} size={16} />
                  <span class="share-item-name" title={it.name}>{it.name}</span>
                  <span class="share-item-size">{humanSize(it.size)}</span>
                  <button
                    class="viewer-btn sm danger"
                    title="删除"
                    onClick={() => handleRemove(it.id)}
                  >
                    <FluentIcon name="delete" size={13} />
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      <div class="share-footer">
        <span class="share-status">{status}</span>
      </div>
    </div>
  );
}
