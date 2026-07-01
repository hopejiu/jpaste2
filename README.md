# jPaste v2

一款 Windows 桌面聚合工具，以剪贴板管理为核心，附加快速启动与工具箱模块。基于 Tauri 2 + Preact + Rust 构建。

## 功能

- **剪贴板历史** — 捕获文本和图片，支持全文/正则搜索、标签筛选、收藏
- **快速启动** — 网页与文件启动目标，支持全局快捷键，行内编辑
- **工具箱** — JSON 查看器、HTTP 调试器 (Curl)、WebSocket 调试器、计算器、编解码工具（base64/URL/Unicode）、时间戳转换器
- **局域网共享** — 本地 HTTP 服务，供同一网络内其他设备访问下载文件/复制文本
- **Toast 通知** — 剪贴板捕获时的悬浮提示，支持一键操作
- **全局快捷键** — 默认 Alt+V 唤出窗口（可自定义）
- **FiloStack 粘贴模式** — 普通、栈（LIFO）、队列（FIFO）三种粘贴策略

## 技术栈

| 层级 | 技术 |
|------|------|
| 前端 | Preact, TypeScript, Vite, wouter |
| 后端 | Rust, Tauri 2, rusqlite |
| 存储 | SQLite + 本地文件 |
| 共享服务 | axum（HTTP 服务） |

## 开发

仅限 **Windows**（依赖 Win32 API）。

```bash
pnpm install          # 安装依赖（pnpm@10, 锁定文件）
pnpm tauri dev        # 启动开发服务器 + Tauri 窗口
```

Vite 运行在 3420 端口，`tauri dev` 自动打开主窗口。

### 运行测试

```bash
pnpm test                       # 前端单元测试（vitest + jsdom）
cargo test --manifest-path src-tauri/Cargo.toml   # Rust 单元测试
```

### 构建发布版

```bash
pnpm tauri build
```

产物输出到 `src-tauri/target/release/bundle/`（MSI、NSIS 安装包、绿色版 exe）。

构建需配置 `TAURI_SIGNING_PRIVATE_KEY` 环境变量用于代码签名。

## 架构

```
src/                    # Preact 前端（TypeScript）
  actions/              # 自动注册的 Action 模块（detect + handler）
  features/             # Viewer 功能（json, curl, ws, calc, decoder, timestamp, ...）
  routes/               # 主页、设置、工具箱、快速启动、Toast、共享
  hooks/                # 共享 hooks（entries, keyboard, events, ...）
src-tauri/src/          # Rust 后端
  command/              # 按领域划分的 Tauri 命令（clipboard, history, share_server, ...）
  service/              # 业务逻辑（history, settings, filostack, ...）
  repository/           # SQLite 数据访问（rusqlite）
  clipboard/            # Win32 剪贴板监听与读取
  monitor/              # 键盘钩子 + Toast 构建器
  model/                # 共享类型定义
```

全局状态由 `AppState` 持有（history、settings、filostack、剪贴板管理器、键盘钩子、启动热键映射）。Tauri 命令是唯一的 IPC 边界，前端通过 `invoke()` 和 Tauri 事件驱动 UI。

多窗口设计：主窗口（`/`）、Toast 窗口（`/toast`，3 秒自动隐藏）、各类型查看器窗口（`/viewer/<type>`）。

## 项目文档

- [`CONTEXT.md`](CONTEXT.md) — 领域术语与架构总览
- [`docs/prd-aggregator.md`](docs/prd-aggregator.md) — 产品需求文档
- [`docs/adr/`](docs/adr/) — 架构决策记录

## 许可证

私有项目 — 保留所有权利。
