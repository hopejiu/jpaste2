import { registerViewerAction } from './viewer-action';

registerViewerAction(
  'curl',
  'HTTP 调试',
  80,
  '/viewer/curl',
  (content: string) => content.trim().toLowerCase().startsWith('curl '),
  true,
);
