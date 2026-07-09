import { useState, useEffect } from 'preact/hooks';
import { useEntryId } from './use-entry-id';
import { api } from '../lib/invoke';
import { copyToClipboard } from '../lib/clipboard';
import { error as logError } from '../lib/logger';

export interface ViewerEntry {
  entryId: number;
  /** Fetched entry content (empty string for blank/toolbox viewers). */
  content: string;
  loading: boolean;
  error: string;
  copy: (text: string) => void;
}

/**
 * Shared entry-loading scaffold for viewer pages.
 *
 * Replaces the repeated `useEntryId() + api.getEntryContent() + logError`
 * boilerplate every viewer page had. Blank/toolbox viewers (entryId <= 0)
 * get empty content with no error; entryId === 0 is reported as invalid.
 */
export function useViewerEntry(): ViewerEntry {
  const entryId = useEntryId();
  const [content, setContent] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  useEffect(() => {
    if (entryId <= 0) {
      setLoading(false);
      if (entryId === 0) setError('无效的条目 ID');
      return;
    }
    setLoading(true);
    setError('');
    api.getEntryContent(entryId)
      .then((data) => {
        setContent(data ?? '');
        setLoading(false);
      })
      .catch((e) => {
        logError('useViewerEntry', e);
        setError(String(e?.message || '获取数据失败'));
        setLoading(false);
      });
  }, [entryId]);

  const copy = (text: string) => copyToClipboard(text);
  return { entryId, content, loading, error, copy };
}
