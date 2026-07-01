import { signal } from '@preact/signals';
import { register } from '../../actions/registry';
import { api } from '../../lib/invoke';
import { info, error as logError } from '../../lib/logger';
import { Modal } from '../../components/modal';
import { FluentIcon } from '../../components/fluent-icon';

// ── State for file:// handler ───────────────────────────────────────

interface FileUriModalState {
  uri: string;
  nativePath: string;
  filename: string;
  parentDir: string;
  existingParent?: string;
  error?: string;
}

const fileUriState = signal<FileUriModalState | null>(null);

// ── Helpers ─────────────────────────────────────────────────────────

/** Convert a file:// URI to a native Windows path. */
function fileUriToNativePath(uri: string): string {
  let path = uri.trim();
  // Strip file:// prefix
  if (path.startsWith('file:///')) {
    path = path.slice(8); // file:///C:/path → C:/path
  } else if (path.startsWith('file://')) {
    path = path.slice(7);
  }
  // Percent-decode
  try {
    path = decodeURIComponent(path);
  } catch { /* keep as-is */ }
  // Convert forward slashes to backslashes
  path = path.replace(/\//g, '\\');
  // Pipe notation: C| → C:
  path = path.replace(/^([A-Za-z])\|/, '$1:');
  return path;
}

// ── Action registration ────────────────────────────────────────────

register({
  id: 'open-url',
  label: '打开链接',
  priority: 80,
  detect: (content: string) => {
    const s = content.trim().toLowerCase();
    return (
      s.startsWith('http://') ||
      s.startsWith('https://') ||
      s.startsWith('ftp://') ||
      s.startsWith('file://')
    );
  },
  handler: async (content: string) => {
    const raw = content.trim();
    const lower = raw.toLowerCase();

    // file://  → folder-style modal (parse to native path)
    if (lower.startsWith('file://')) {
      const nativePath = fileUriToNativePath(raw);
      const idx = nativePath.lastIndexOf('\\');
      const parentDir = idx > 0 ? nativePath.substring(0, idx) : '';
      const filename = idx >= 0 ? nativePath.substring(idx + 1) : nativePath;

      try {
        const type: string = await api.invoke('get_path_type', { path: nativePath });
        if (type === 'dir') {
          await api.invoke('open_in_explorer', { path: nativePath });
        } else if (type === 'file') {
          fileUriState.value = { uri: raw, nativePath, filename, parentDir };
        } else {
          // Walk up looking for existing ancestor
          let ancestor = parentDir;
          let found: string | undefined;
          while (ancestor.length > 3) {
            const t: string = await api.invoke('get_path_type', { path: ancestor });
            if (t !== 'not_found') { found = ancestor; break; }
            const next = ancestor.lastIndexOf('\\');
            if (next <= 2) break;
            ancestor = ancestor.substring(0, next);
          }
          if (found) {
            fileUriState.value = { uri: raw, nativePath, filename, parentDir, existingParent: found };
          } else {
            fileUriState.value = {
              uri: raw, nativePath, filename, parentDir,
              error: `未找到路径：${nativePath}\n请检查路径是否正确`,
            };
          }
        }
      } catch (e) {
        logError('action:open-url', e);
      }
      return;
    }

    // http:// https:// ftp:// → open with system default browser
    info('action:open-url', { url: raw });
    try {
      await api.openUrl(raw);
    } catch (e) {
      logError('action:open-url', e);
    }
  },
});

// ── Modal component for file:// ─────────────────────────────────────

export function FileUriModal() {
  const state = fileUriState.value;
  if (!state) return null;

  const close = () => { fileUriState.value = null; };

  if (state.error) {
    return (
      <Modal open title="路径不存在" onClose={close}>
        <p class="folder-modal-path">{state.error}</p>
        <div class="folder-modal-btns">
          <button class="folder-modal-btn" onClick={close}>确定</button>
        </div>
      </Modal>
    );
  }

  if (state.existingParent) {
    return (
      <Modal open title="路径不存在" onClose={close}>
        <p class="folder-modal-path">{state.filename}</p>
        <p class="folder-modal-desc">未找到该路径，是否打开上层目录？</p>
        <p class="folder-modal-ancestor"><FluentIcon name="folder" size={16} /> {state.existingParent}</p>
        <div class="folder-modal-btns">
          <button class="folder-modal-btn" onClick={close}>取消</button>
          <button
            class="folder-modal-btn primary"
            onClick={() => {
              api.invoke('open_in_explorer', { path: state.existingParent! }).catch((e) => logError('open-url:openParent', e));
              close();
            }}
          >打开上层目录</button>
        </div>
      </Modal>
    );
  }

  // File found — choice between folder and file
  return (
    <Modal open title="打开路径" onClose={close}>
      <p class="folder-modal-path">{state.filename}</p>
      <p class="folder-modal-desc">{state.nativePath}</p>
      <p class="folder-modal-desc">选择操作方式：</p>
      <div class="folder-modal-btns">
        <button class="folder-modal-btn" onClick={() => {
          api.invoke('open_in_explorer', { path: state.parentDir }).catch((e) => logError('open-url:openDir', e));
          close();
        }}>打开所在目录</button>
        <button class="folder-modal-btn primary" onClick={() => {
          api.invoke('open_in_explorer', { path: state.nativePath }).catch((e) => logError('open-url:openFile', e));
          close();
        }}>打开文件</button>
      </div>
    </Modal>
  );
}
