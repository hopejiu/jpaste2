import { registerViewerAction } from '../../actions/viewer-action';

registerViewerAction(
  'timestamp',
  '时间戳转换',
  30,
  '/viewer/timestamp',
  (content: string) => {
    const s = content.trim();
    if (s.length !== 10 && s.length !== 13) return false;
    if (!/^\d+$/.test(s)) return false;
    const d = new Date(parseInt(s) * (s.length === 13 ? 1 : 1000));
    return d.getFullYear() >= 2000 && d.getFullYear() <= 2100;
  },
  false,
);
