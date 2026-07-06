/// <reference types="vite/client" />

import { render } from 'preact';
import { App } from './app';
import { info, error, setComponent } from './lib/logger';

setComponent('main');

// Global error handler
window.addEventListener('error', (e) => error('Global error:', e.message));
window.addEventListener('unhandledrejection', (e) => error('Unhandled rejection:', e.reason));

// Determine the Tauri window label
let windowLabel: string | null = null;
try {
  windowLabel = (window as any).__TAURI__?.webviewWindow?.getCurrent?.()?.label ?? null;
} catch { /* ignore */ }

// Detect toast window via sessionStorage marker (survives webview reloads
// when __TAURI__ IPC bridge is not yet available).
const isToastWindow = sessionStorage.getItem('__TOAST_WINDOW__') === '1';

// Hash resolution
const hashFromUrl = window.location.hash && window.location.hash !== '#'
  ? window.location.hash.replace(/^#/, '')
  : null;
const hashFromInit = (window as any).__INITIAL_HASH__ as string | undefined;
let resolvedHash = hashFromUrl || hashFromInit || null;

if (!resolvedHash && windowLabel) {
  const m = windowLabel.match(/^(json|image|curl|ws|calc|decoder|timestamp)-viewer-(\d+)$/);
  if (m) resolvedHash = `/viewer/${m[1]}?id=${m[2]}`;
}

if (resolvedHash) {
  window.location.hash = resolvedHash;
} else if (windowLabel === 'toast-0' || isToastWindow) {
  window.location.hash = (window as any).__INITIAL_HASH__ || '/toast?title=jPaste&message=';
}

try {
  const root = document.getElementById('app');
  if (!root) {
    error('#app element not found!');
    document.body.innerHTML =
      '<div style="padding:20px;color:red;">Error: #app element not found</div>';
  } else {
    info('rendering App...');
    render(<App />, root);
    info('render complete');
  }
} catch (err) {
  error('Fatal error:', err);
  document.body.innerHTML = `<pre style="padding:20px;color:red;">Error: ${err instanceof Error ? err.message : String(err)}\n${err instanceof Error ? err.stack : ''}</pre>`;
}
