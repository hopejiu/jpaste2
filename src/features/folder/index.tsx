import { signal } from '@preact/signals';
import { register } from '../../actions/registry';
import { api } from '../../lib/invoke';
import { info, error as logError } from '../../lib/logger';
import { Modal } from '../../components/modal';
import { FluentIcon } from '../../components/fluent-icon';

// ── State ────────────────────────────────────────────────────────────

interface FolderModalState {
  path: string;            // the full original path
  filename: string;        // display name (last component)
  parentDir: string;       // immediate parent directory
  existingParent?: string; // nearest existing ancestor (for "not_found" recovery)
  error?: string;          // if set, show error only
}

export const folderState = signal<FolderModalState | null>(null);

// ── Action registration ──────────────────────────────────────────────

register({
  id: 'folder',
  label: '打开路径',
  priority: 70,
  detect: (content: string) => {
    const s = content.trim();
    return /^[A-Za-z]:\\/.test(s) || s.startsWith('\\\\');
  },
  handler: async (content: string) => {
    const path = content.trim();
    info('action:folder', { path });

    const idx = path.lastIndexOf('\\');
    const parentDir = idx > 2 ? path.substring(0, idx) : '';
    const filename = idx >= 0 ? path.substring(idx + 1) : path;

    try {
      const type: string = await api.invoke('get_path_type', { path });
      if (type === 'dir') {
        await api.invoke('open_in_explorer', { path });
      } else if (type === 'file') {
        folderState.value = { path, filename, parentDir };
      } else {
        // Path doesn't exist — walk up looking for an existing ancestor
        let ancestor = parentDir;
        let found: string | undefined;
        while (ancestor.length > 3) {
          const t: string = await api.invoke('get_path_type', { path: ancestor });
          if (t !== 'not_found') { found = ancestor; break; }
          const next = ancestor.lastIndexOf('\\');
          if (next <= 2) break; // stop at drive root (X:\)
          ancestor = ancestor.substring(0, next);
        }
        if (found) {
          folderState.value = { path, filename, parentDir, existingParent: found };
        } else {
          folderState.value = {
            path, filename, parentDir,
            error: `未找到路径：${path}\n请检查路径是否正确`,
          };
        }
      }
    } catch (e) {
      logError('action:folder', e);
    }
  },
});

// ── Modal component ──────────────────────────────────────────────────

export function FolderModal() {
  const state = folderState.value;
  if (!state) return null;

  const close = () => { folderState.value = null; };

  // Error state — show error message and close only
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

  const filename = state.filename;
  const parentDir = state.parentDir;

  // Existing ancestor found (path doesn't exist but some parent does)
  if (state.existingParent) {
    return (
      <Modal open title="路径不存在" onClose={close}>
        <p class="folder-modal-path">{filename}</p>
        <p class="folder-modal-desc">
          未找到该路径，是否打开上层目录？
        </p>
        <p class="folder-modal-ancestor"><FluentIcon name="folder" size={16} /> {state.existingParent}</p>
        <div class="folder-modal-btns">
          <button class="folder-modal-btn" onClick={close}>取消</button>
          <button
            class="folder-modal-btn primary"
            onClick={() => {
              api.invoke('open_in_explorer', { path: state.existingParent! }).catch((e) => logError('folder:openParent', e));
              folderState.value = null;
            }}
          >打开上层目录</button>
        </div>
      </Modal>
    );
  }

  // File found — choice between folder and file
  return (
    <Modal open title="打开路径" onClose={close}>
      <p class="folder-modal-path">{filename}</p>
      <p class="folder-modal-desc">选择操作方式：</p>
      <div class="folder-modal-btns">
        <button class="folder-modal-btn" onClick={() => {
          api.invoke('open_in_explorer', { path: parentDir }).catch((e) => logError('folder:openDir', e));
          folderState.value = null;
        }}>打开所在目录</button>
        <button class="folder-modal-btn primary" onClick={() => {
          api.invoke('open_in_explorer', { path: state.path }).catch((e) => logError('folder:openFile', e));
          folderState.value = null;
        }}>打开文件</button>
      </div>
    </Modal>
  );
}
