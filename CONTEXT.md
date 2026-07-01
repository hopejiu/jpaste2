# jPaste v2 — Domain Glossary

A Windows clipboard manager built with Rust + egui.

## Domain Terms

### Clipboard Entry
A single copy operation captured from the system clipboard. Each entry is uniquely identified by `content_hash` — the SHA-256 of the trimmed `CF_UNICODETEXT` content. For image-only copies (no text), the hash is computed from the decoded image bytes instead. Deduplication refreshes `updated_at` but does not create a duplicate.

### Clipboard Format
A format-specific payload attached to a **Clipboard Entry**. Each clipboard operation may carry multiple formats simultaneously — `CF_UNICODETEXT` (plain text), `CF_HTML`, `CF_DIB` (image). Text formats are stored inline; image formats saved to the **Image Store** with only the file path in the database.

### Clipboard Source
The application that wrote the current clipboard content. Captured via `GetClipboardOwner()` → `GetWindowThreadProcessId()` → `QueryFullProcessImageNameW()` for the executable path, plus `GetWindowTextW()` for the window title.

### Paste Order
A two-state setting (`paste_order: "normal" | "queue"`) controlling how `Ctrl+V` consumes recently captured items. Queue mode installs a `WH_KEYBOARD_LL` global hook that intercepts Ctrl+V, pops from the FIFO queue, writes to the system clipboard, and lets the keystroke pass. Self-writes are guarded by a hash-based 5-second TTL.

### Clipboard Queue
A sub-mode of **Paste Order** (`"queue"`). Items consumed FIFO (First In, First Out). Copy order 1,2,3 → paste order 1,2,3.

### Image Store
File directory at `{data_dir}/images/{YYYY-MM-DD}/{uuid}.png` for clipboard image payloads. Organized by date for easy cleanup. Only PNG is stored (no raw DIB) — `clipboard-rs` handles format conversion.

### Search Sort Order
Two options: `updated_at DESC` (default) and `content_length ASC`. Set via settings dropdown.

### Entry Tag
Classification label assigned at capture time. Determined by format presence and content pattern matching:

| Tag | Bit | Determination |
|-----|-----|---------------|
| `text` | 1 | Has `CF_UNICODETEXT` and no image/file-path formats |
| `image` | 4 | Has `CF_DIB` or `CF_DIBV5` |
| `url` | 8 | Text starts with `http://` or `https://` |
| `file` | 16 | Has `CF_HDROP` or text matches Windows path pattern |

### Favorite
A user-assigned marker stored as `is_favorite BOOLEAN` on `clipboard_entry`. Not affected by capture-time tag recomputation.

### Tag Mask
A bitmask on `clipboard_entry.tag_mask` encoding multiple **Entry Tags** via bitwise OR. Filtered with `tag_mask & ? != 0` in SQL. 0 means "show all".

### Cursor Pagination
Compound cursor `(updated_at, id)`, 20 per page, auto-loaded when user scrolls to bottom. First request uses zero-values. Timestamps at millisecond precision.

### Data Directory
`%APPDATA%/jPastev2/` — contains `clipboard.db`, `images/`, `settings.json`, `jpaste.log`.

## Key Behaviors

- **Close button** hides to system tray, does NOT quit
- **Global hotkey** (default `Alt+V`) toggles window visibility
- **Lose focus** → auto-hide, unless pinned
- **Window** starts hidden if `start_minimized`; otherwise centered on first show
- **Clipboard monitoring** via `clipboard-rs` `ClipboardWatcherContext` (background thread, event-driven)
- **Copy action** writes to clipboard + hides window
- **Paste action** removed; only queue-mode hook provides automatic pasting
- **Queue auto-exit** on non-text capture (image/file) automatically reverts to "normal" mode
- **Cleanup** removes entries older than `retain_days` (favorites exempted)
- **Infinite scroll** loads 20 entries per page, auto-triggers on scroll-to-bottom
