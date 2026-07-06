import { registerViewerAction } from './viewer-action';

registerViewerAction(
  'ws',
  'WS 调试',
  80,
  '/viewer/ws',
  (content: string) => {
    const s = content.trim();
    return s.startsWith('ws://') || s.startsWith('wss://');
  },
  true,
);
