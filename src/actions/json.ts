import { registerViewerAction } from './viewer-action';

registerViewerAction(
  'json',
  'JSON 查看',
  90,
  '/viewer/json',
  (content: string) => {
    const s = content.trim();
    return (s.startsWith('{') || s.startsWith('[')) && tryParseJson(s);
  },
  true,
);

function tryParseJson(s: string): boolean {
  try { JSON.parse(s); return true; }
  catch { return false; }
}
