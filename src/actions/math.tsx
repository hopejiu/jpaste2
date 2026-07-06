import { registerViewerAction } from './viewer-action';

registerViewerAction(
  'math',
  '计算',
  60,
  '/viewer/calc',
  (content: string) => {
    const s = content.trim();
    if (!s) return false;
    return /^[\d+\-*/().%\s]+$/.test(s) && /[+\-*/%]/.test(s);
  },
  false,
);
