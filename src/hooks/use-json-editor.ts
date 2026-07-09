import { useRef, useCallback } from 'preact/hooks';

/**
 * Hook for dynamically loading and managing a JSONEditor instance.
 * Returns imperative methods for both full-page (JsonViewPage) and
 * toggle-on-demand (CurlViewPage) scenarios.
 */
export function useJsonEditor() {
  const containerRef = useRef<HTMLDivElement>(null);
  const editorRef = useRef<any>(null);

  const updateJson = useCallback(async (data: any, mode?: 'tree' | 'code') => {
    const container = containerRef.current;
    if (!container) throw new Error('编辑器容器未就绪');
    // ponytail: `data` can be `false`/`0` etc. — only skip explicit null/undefined.
    if (data === null || data === undefined) throw new Error('无效的 JSON 数据：null 或 undefined');

    // Update existing.
    if (editorRef.current) {
      editorRef.current.update(data);
      return;
    }

    // Lazy-load and create.
    let JSONEditor: any;
    try {
      const mod = await import('jsoneditor');
      JSONEditor = mod.default;
      await import('jsoneditor/dist/jsoneditor.css');
    } catch (e) {
      throw new Error('JSON 编辑器加载失败: ' + (e instanceof Error ? e.message : String(e)));
    }

    const savedMode: 'tree' | 'code' = (() => {
      try {
        const m = localStorage.getItem('jpaste-json-mode');
        return (m === 'tree' || m === 'code') ? m : 'tree';
      } catch { return 'tree'; }
    })();

    try {
      const editor = new JSONEditor(container, {
        mode: mode || savedMode,
        modes: ['tree', 'code'],
        mainMenuBar: true,
        navigationBar: true,
        statusBar: true,
        search: true,
        history: true,
        indentation: 2,
        sortObjectKeys: false,
        limitDragging: false,
        onModeChange: (newMode: string) => {
          try { localStorage.setItem('jpaste-json-mode', newMode); } catch { /* ignore */ }
        },
      }, data);
      editorRef.current = editor;
    } catch (e) {
      throw new Error('JSON 编辑器创建失败: ' + (e instanceof Error ? e.message : String(e)));
    }
  }, []);

  const destroyEditor = useCallback(() => {
    if (editorRef.current) {
      editorRef.current.destroy();
      editorRef.current = null;
    }
  }, []);

  return { containerRef, updateJson, destroyEditor };
}
