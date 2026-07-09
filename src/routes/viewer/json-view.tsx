import { useEffect, useState } from 'preact/hooks';
import { useJsonEditor } from '../../hooks/use-json-editor';
import { useViewerEntry } from '../../hooks/use-viewer-entry';
import { info as logInfo, error as logError } from '../../lib/logger';

export function JsonViewPage() {
  const { entryId, content, loading, error: fetchError } = useViewerEntry();
  const [parseError, setParseError] = useState('');
  const { containerRef, updateJson, destroyEditor } = useJsonEditor();

  logInfo('JsonViewPage', { entryId, hash: window.location.hash });

  // Capture-phase Escape: fires before jsoneditor's internal handlers
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') { e.preventDefault(); window.history.back(); }
    };
    document.addEventListener('keydown', handler, true);
    return () => document.removeEventListener('keydown', handler, true);
  }, []);

  useEffect(() => {
    logInfo('JsonViewPage:content', { entryId, len: content?.length });
    if (entryId <= 0) {
      if (entryId === 0) return; // hook already reported invalid id
      // id=-1: blank viewer (toolbox) — load editor with empty object so user can paste JSON
      updateJson({}).catch((e) => logError('JsonViewPage:initEmpty', e));
      return;
    }
    if (!content) { setParseError('条目内容为空'); return; }
    let parsed: unknown;
    try {
      parsed = JSON.parse(content);
    } catch (e: any) {
      setParseError('JSON 解析失败: ' + (e?.message || e));
      return;
    }
    setParseError('');
    updateJson(parsed).catch((e: any) =>
      setParseError('JSON 解析失败: ' + (e?.message || e)),
    );
  }, [content]);

  useEffect(() => {
    return () => destroyEditor();
  }, []);

  const error = fetchError || parseError;

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
