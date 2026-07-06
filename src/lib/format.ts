// ── Format utilities matching old jPaste ──────────────────────────────

const pad2 = (n: number) => String(n).padStart(2, '0');

export function formatBytes(bytes: number): string {
  if (!bytes || bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const val = (bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0);
  return `${val} ${units[i]}`;
}

export function formatTime(ts: number | string): { rel: string; abs: string } {
  if (ts === undefined || ts === null || ts === '') return { rel: '', abs: '' };
  try {
    // Backend stores timestamps as Unix ms (UTC). Number → direct,
    // string → old-format ISO string fallback for pre-migration data.
    const d = typeof ts === 'number' ? new Date(ts) : new Date(ts.replace(' ', 'T') + 'Z');
    if (isNaN(d.getTime())) return { rel: String(ts), abs: String(ts) };
    const now = new Date();
    const diff = now.getTime() - d.getTime();
    const abs = `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
    let rel: string;
    if (diff < 60000) rel = '刚刚';
    else if (diff < 3600000) rel = `${Math.floor(diff / 60000)} 分钟前`;
    else if (diff < 86400000) rel = `${Math.floor(diff / 3600000)} 小时前`;
    else if (diff < 604800000) rel = `${Math.floor(diff / 86400000)} 天前`;
    else if (diff < 2592000000) rel = `${Math.floor(diff / 604800000)} 周前`;
    else rel = abs.slice(0, 10);
    return { rel, abs };
  } catch {
    return { rel: String(ts), abs: String(ts) };
  }
}

export function previewContent(content: string): string {
  if (!content) return '';
  const lines = content.split('\n');
  const preview = lines.slice(0, 3).join('\n');
  if (preview.length > 300) return preview.slice(0, 300) + '...';
  if (lines.length > 3) return preview + '\n...';
  if (content.length > 300) return content.slice(0, 300) + '...';
  return content;
}
