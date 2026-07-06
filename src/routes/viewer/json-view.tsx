import { useEffect, useState, useRef } from 'preact/hooks';
import { api } from '../../lib/invoke';
import { useJsonEditor } from '../../hooks/use-json-editor';
import { useEntryId } from '../../hooks/use-entry-id';
import { info as logInfo, error as logError } from '../../lib/logger';

export function JsonViewPage() {
  const entryId = useEntryId();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const fetchedRef = useRef(false);
  const { containerRef, updateJson, destroyEditor } = useJsonEditor();

  logInfo('JsonViewPage', { entryId });

  // Capture-phase Escape: fires before jsoneditor's internal handlers
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') { e.preventDefault(); window.history.back(); }
    };
    document.addEventListener('keydown', handler, true);
    return () => document.removeEventListener('keydown', handler, true);
  }, []);

  useEffect(() => {
    if (!entryId) {
      setLoading(false);
      setError('无效的条目 ID');
      return;
    }
    if (fetchedRef.current) return;
    fetchedRef.current = true;

    api.getEntryContent(entryId)
      .then((data) => {
        if (!data) {
          setError('条目内容为空');
          setLoading(false);
        } else {
          try {
            updateJson(JSON.parse(data));
          } catch (e: any) {
            setError('JSON 解析失败: ' + e.message);
          }
          setLoading(false);
        }
      })
      .catch((e) => {
        logError('JsonViewPage', e);
        setError(String(e?.message || '获取数据失败'));
        setLoading(false);
      });
  }, [entryId]);

  useEffect(() => {
    return () => destroyEditor();
  }, []);

  return (
    <div class="viewer-page" style={{ position: 'relative' }}>
      {/* Hide the "powered by ace" link in code mode */}
      <style>{'.jsoneditor-poweredBy { display: none !important; }'}</style>
      <div ref={containerRef} style={{ width: '100%', height: '100vh' }} />

      {loading && (
        <div class="viewer-loading" style={{ position: 'absolute', inset: 0, zIndex: 1, background: 'var(--bg-primary)' }}>
          加载中...
        </div>
      )}

      {error && !loading && (
        <div class="viewer-error" style={{ position: 'absolute', inset: 0, zIndex: 1, background: 'var(--bg-primary)' }}>
          <div style={{ textAlign: 'center' }}>
            <p>错误: {error}</p>
            <p style={{ marginTop: 8, fontSize: 12, color: 'var(--text-muted)' }}>Entry ID: {entryId || '(无)'}</p>
          </div>
        </div>
      )}
    </div>
  );
}
