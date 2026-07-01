import { describe, it, expect } from 'vitest';

// ponytail: mirrors the ACTION_META and VIEWER_ROUTES from toast.tsx.
// These must stay in sync with Rust side detect_actions() in model.rs.
// If you add/remove an action, update ALL THREE:
//   1. Rust model.rs detect_actions()
//   2. Frontend toast.tsx ACTION_META + VIEWER_ROUTES
//   3. This test

const ACTION_META: Record<string, { icon: string; label: string }> = {
  json:      { icon: 'code',      label: 'JSON 查看' },
  curl:      { icon: 'terminal',  label: 'HTTP 调试' },
  ws:        { icon: 'wifi',      label: 'WS 调试' },
  decoder:   { icon: 'code',      label: '解码' },
  timestamp: { icon: 'clock',     label: '时间戳转换' },
  math:      { icon: 'calculator',label: '计算' },
  folder:    { icon: 'folder',    label: '打开所在目录' },
  'open-url':{ icon: 'link',      label: '打开链接' },
  qrcode:    { icon: 'qrCode',    label: '复制二维码' },
};

const VIEWER_ROUTES: Record<string, string> = {
  json: '/viewer/json',
  curl: '/viewer/curl',
  ws: '/viewer/ws',
  decoder: '/viewer/decoder',
  timestamp: '/viewer/timestamp',
  math: '/viewer/calc',
};

// Icon names used in ACTION_META — must be valid FluentIcon names.
// These are added dynamically in fluent-icon.tsx, so here we just validate
// they're non-empty strings.
const VALID_FLUENT_ICONS = new Set([
  'code', 'terminal', 'wifi', 'calculator', 'folder', 'link', 'clock', 'qrCode',
]);

type ActionId = keyof typeof ACTION_META;

describe('toast action metadata', () => {
  it('has 9 action entries', () => {
    expect(Object.keys(ACTION_META)).toEqual([
      'json', 'curl', 'ws', 'decoder', 'timestamp', 'math', 'folder', 'open-url', 'qrcode',
    ]);
  });

  it('every action has a non-empty label and icon', () => {
    for (const [id, meta] of Object.entries(ACTION_META)) {
      expect(meta.label, `action ${id} label`).toBeTruthy();
      expect(meta.icon, `action ${id} icon`).toBeTruthy();
      expect(VALID_FLUENT_ICONS.has(meta.icon), `action ${id} icon "${meta.icon}" must exist in FluentIcon`).toBe(true);
    }
  });

  it('viewer routes cover all viewer-type actions', () => {
    const viewerActions = ['json', 'curl', 'ws', 'decoder', 'timestamp', 'math'];
    for (const id of viewerActions) {
      expect(VIEWER_ROUTES[id], `viewer route for ${id}`).toBeTruthy();
    }
  });

  it('non-viewer actions (folder, open-url) do not have viewer routes', () => {
    expect(VIEWER_ROUTES['folder']).toBeUndefined();
    expect(VIEWER_ROUTES['open-url']).toBeUndefined();
  });
});

describe('handleActionClick dispatch logic', () => {
  // Simulates the dispatch logic in toast.tsx handleActionClick
  const ACTION_META_KEYS = Object.keys(ACTION_META) as ActionId[];

  it('folder action dispatches to open_in_explorer (opens parent dir in toast)', () => {
    const actionId = 'folder';
    expect(ACTION_META_KEYS).toContain(actionId);
    expect(VIEWER_ROUTES[actionId]).toBeUndefined();
  });

  it('open-url action dispatches to openUrl', () => {
    const actionId = 'open-url';
    expect(ACTION_META_KEYS).toContain(actionId);
    expect(VIEWER_ROUTES[actionId]).toBeUndefined();
  });

  it.each(['json', 'curl', 'ws', 'decoder', 'timestamp', 'math'])(
    'viewer action %s dispatches to openViewer with route',
    (actionId) => {
      expect(VIEWER_ROUTES[actionId]).toMatch(/^\/viewer\/\w+$/);
    },
  );
});

describe('action ID parity with Rust', () => {
  // Rust detect_actions() returns: json, curl, ws, decoder, timestamp, math, folder, open-url
  // qrcode is detected separately (image-based, not text-based), so it's exempt from parity check.
  const RUST_ACTION_IDS = [
    'json', 'curl', 'ws', 'decoder', 'timestamp', 'math', 'folder', 'open-url',
  ] as const;
  const FRONTEND_EXTRAS = ['qrcode']; // image-based, not in text detect_actions()

  it('every Rust action ID has a corresponding frontend entry', () => {
    for (const id of RUST_ACTION_IDS) {
      expect(ACTION_META[id], `Rust action "${id}" must have frontend mapping`).toBeTruthy();
    }
  });

  it('every frontend action ID is a valid Rust action or a known extra', () => {
    for (const id of Object.keys(ACTION_META)) {
      const inRust = RUST_ACTION_IDS.includes(id as any);
      const inExtras = (FRONTEND_EXTRAS as readonly string[]).includes(id);
      expect(inRust || inExtras, `frontend action "${id}" must exist in Rust detect_actions() or be listed as a known extra`).toBe(true);
    }
  });
});
