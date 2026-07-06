// ── Domain types shared between frontend and backend ───────────────────

export interface Entry {
  id: number;
  content_hash: string;
  content: string;
  content_preview: string;
  image_path: string;
  thumb_path: string;
  has_image: boolean;
  tag_mask: number;
  is_favorite: boolean;
  content_length: number;
  copy_count: number;
  qr_text: string;
  created_at: number;
  updated_at: number;
}

export interface QueryResult {
  entries: Entry[];
  has_more: boolean;
}

export interface Settings {
  hotkey: string;
  retain_days: number;
  auto_start: boolean;
  start_minimized: boolean;
  notify_enabled: boolean;
  paste_order: string;
  action_config: Record<string, unknown>;
  sort_field: string;
  sort_order: string;
  auto_clear_search: boolean;
  auto_clear_seconds: number;
  auto_hide_after_copy: boolean;
  default_action: string; // "copy" | "paste"
  center_on_show: boolean;
  auto_fav_on_copy_count: boolean;
  auto_fav_threshold: number;
}

export interface FiloStatus {
  mode: string;
  mode_name: string;
  enabled: boolean;
  item_count: number;
  items: string[];
}

export interface Stats {
  count: number;
  total_bytes: number;
  image_bytes: number;
}

export interface CleanupResult {
  deleted: number;
}

export interface ClipboardUpdatePayload {
  id: number;
  content_preview: string;
  tag_mask: number;
  copy_count: number;
  auto_favorited: boolean;
}

// ── Tag constants ──────────────────────────────────────────────────────

export const TAG_TEXT = 1 << 0;
export const TAG_IMAGE = 1 << 2;
export const TAG_URL = 1 << 3;
export const TAG_FILE = 1 << 4;
export const TAG_FAVORITE = 1 << 5;

export const TAG_QR = 1 << 6;

export const TAG_NAMES: Record<number, string> = {
  [TAG_TEXT]: '全部',
  [TAG_IMAGE]: '图片',
  [TAG_URL]: '网址',
  [TAG_FILE]: '文件',
  [TAG_FAVORITE]: '收藏',
};

export const TAG_TABS = [
  { mask: 0, label: '全部' },
  { mask: TAG_TEXT, label: '文本' },
  { mask: TAG_IMAGE, label: '图片' },
  { mask: TAG_URL, label: '网址' },
  { mask: TAG_FILE, label: '文件' },
  { mask: TAG_FAVORITE, label: '收藏' },
];
