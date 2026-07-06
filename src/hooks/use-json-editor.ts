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
    if (!container || !data) return;

    // Update existing.
    if (editorRef.current) {
      editorRef.current.update(data);
      return;
    }

    // Lazy-load and create.
    const [{ default: JSONEditor }, _css] = await Promise.all([
      import('jsoneditor'),
      import('jsoneditor/dist/jsoneditor.css'),
    ]);

    const savedMode: 'tree' | 'code' = (() => {
      try {
        const m = localStorage.getItem('jpaste-json-mode');
        return (m === 'tree' || m === 'code') ? m : 'tree';
      } catch { return 'tree'; }
    })();

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
  }, []);

  const destroyEditor = useCallback(() => {
    if (editorRef.current) {
      editorRef.current.destroy();
      editorRef.current = null;
    }
  }, []);

  return { containerRef, updateJson, destroyEditor };
}
