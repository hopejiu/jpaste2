# jPaste v2 — Domain Glossary

A Windows clipboard manager rebuilt from jPaste (Go + Wails3 + React + JS) using Rust + Tauri + Preact + TypeScript.

## Domain Terms

参见 jPaste v1 的完整术语表：[旧项目 CONTEXT.md](../CONTEXT-old.md)
（v2 保持相同领域语义，所有域术语、位掩码、事件名、存储结构均不变。）

### 与 v1 的关键差异

- **Rust 后端**代替 Go 后端 — Tauri 命令代替 Wails Bindings
- **Preact + TypeScript** 代替 React + JavaScript
- **Tauri 事件系统**代替 Wails Events
- **rusqlite** 代替 `modernc.org/sqlite`
- **单实例限制**通过 OS 级别的 Mutex（`SingleInstance` 或命名互斥体）
- **Toast** 从隐藏→显示的 WebView2 窗口改为每次创建→销毁的 Tauri 物理窗口
- **来源应用追踪**（`source_exe`, `source_title`）— **已放弃**
- **存储简化**：单表 `entries` 代替 v1 的 `clipboard_entry` + `clipboard_format` 双表
- **内容格式**：仅支持纯文本（CF_UNICODETEXT）和图片（CF_DIB/DIBV5），不存储 RTF/HTML/CF_HDROP

## Architecture (v2)

```
┌──────────────────────────────────────────────────────┐
│  Preact Frontend (Tauri WebView)                      │
│  ┌──────────────┐ ┌────────────────┐ ┌──────────┐    │
│  │  MainPage     │ │ SettingsPage   │ │ Viewer*  │    │
│  │  (条目列表)   │ │ (设置页)       │ │ (多窗口) │    │
│  └──────┬───────┘ └────────────────┘ └──────────┘    │
│         ↕ invoke() / Tauri Events                     │
├──────────────────────────────────────────────────────┤
│  Rust Backend                                         │
│  ┌──────────┐ ┌──────────────────┐ ┌────────────┐    │
│  │clipboard │ │ service/history  │ │ viewer/    │    │
│  │(Win32)   │ │ service/settings │ │ service/*  │    │
│  │          │ │ service/filostack│ │ action/    │    │
│  └──────────┘ └────────┬─────────┘ └────────────┘    │
│                        ↕                              │
│  ┌──────────┐ ┌───────┴────────┐                     │
│  │ repository (SQLite) │   util/                     │
│  └─────────────────────┘                              │
├──────────────────────────────────────────────────────┤
│  SQLite + settings.json + 图片文件存储                  │
│  系统托盘 + 全局热键                                    │
└──────────────────────────────────────────────────────┘

(*) Viewer 窗口通过 Tauri 多窗口 API 创建，每个独立的 WebView
```

## Module Layout

```
src/
├── lib.rs              # Tauri Builder + invoke_handler 注册 + 托盘
├── main.rs             # Windows 子系统入口
├── clipboard/          # clipboard-rs 封装（监听、读写、粘贴模拟）
├── hook/               # WH_KEYBOARD_LL 键盘钩子（Ctrl+V 拦截）
├── repository/         # SQLite 数据访问 (rusqlite)，单表 entries
├── model/              # 领域类型（Entry, TagMask 等）
├── service/
│   ├── mod.rs
│   ├── history.rs      # 历史记录 CRUD + 去重 + 搜索 + 分页 + 清理
│   ├── settings.rs     # settings.json 读写 + 变更通知
│   ├── filostack.rs    # 粘贴队列（QueueStrategy FIFO + 键盘钩子集成）
│   ├── fileop.rs       # 文件操作（资源管理器打开）
│   └── toast.rs        # Toast 通知（Tauri 物理窗口，创建→显示→销毁）
├── viewer/             # 多窗口查看器（JSON/Image/Curl/WS）
├── action/             # 内容识别操作模块
└── util/               # 哈希、截断、DIB→BMP、SelfWriteTracker
```

## Dependencies

| Crate | 用途 |
|-------|------|
| `tauri` 2 | 框架核心 |
| `tauri-plugin-global-shortcut` | 全局热键（Alt+V） |
| `tauri-plugin-opener` | 浏览器打开、资源管理器打开 |
| `clipboard-rs` 0.3.5 | 剪贴板读写 + 监听（纯文本、图片、文件列表检测） |
| `windows` | WH_KEYBOARD_LL 键盘钩子 + keybd_event 粘贴模拟 |
| `rusqlite` | SQLite 数据访问 |
| `serde` / `serde_json` | 序列化 |
| `uuid` | 图片文件名生成 |
| `image` | 图片编码/解码（由 clipboard-rs 间接引入） |

## Design Patterns (v2)

| Pattern | Location | Usage |
|---------|----------|-------|
| **Repository** | `repository/` | 单一 Repository 结构体拥有所有 SQLite 查询 |
| **Strategy** | `service/filostack` | PasteStrategy 接口 + QueueStrategy 实现 |
| **Command** | Tauri `#[tauri::command]` | 每个 Tauri 命令对应一个功能操作 |
| **Event-Driven** | Tauri Events | 后端 emit 事件，前端 listen |
| **Observer** | `service/settings` | 设置变更时回调通知依赖方 |
