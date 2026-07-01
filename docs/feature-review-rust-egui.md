# jPaste v2 (Rust + egui) 功能梳理与重构难度评估

> 基于原项目 `F:\work\jpaste` (Go + Wails3 + React + TS) 重构为 Rust + egui 的可行性分析。
> 评估日期：2026-07-01

---

## 目录

1. [核心基础能力](#1-核心基础能力)
2. [UI 层功能](#2-ui-层功能)
3. [保留的高级功能](#3-保留的高级功能)
4. [快捷键与热键](#4-快捷键与热键)
5. [系统集成](#5-系统集成)
6. [存储层](#6-存储层)
7. [其他 / 工具类](#7-其他--工具类)
8. [总结](#8-总结)
9. [依赖清单](#9-依赖清单-cargotoml)

---

## 1. 核心基础能力

### 1.1 剪贴板事件监听 (WM_CLIPBOARDUPDATE)

| 项目 | 说明 |
|------|------|
| **原实现** | 消息窗口 (`HWND_MESSAGE`) + `AddClipboardFormatListener`，纯 Win32 API，`internal/clipboard/clipboard_windows.go` |
| **关键细节** | 独线程消息泵、延迟渲染重试 (8 次, 共 ~5.5s)、OLE IDataObject 回退 (Office/Chromium 等)、哈希去重 |
| **Rust 方案** | `clipboard-rs` 的 `ClipboardWatcherContext` + `ClipboardHandler` trait |
| **难度** | ⭐⭐ 较易 |
| **备注** | `clipboard-rs` 内置跨平台剪贴板监听，通过 `ClipboardHandler::on_clipboard_change(&mut self)` 回调通知变化。底层封装了 WM_CLIPBOARDUPDATE / 轮询（平台自适应），不需要自己写消息泵。监听在后台独立线程，egui 主线程不受影响。**注意**：回调不携带变化的内容，需要在回调里自行 `get_text()` / `has(Image)` 检查并比对。 |

### 1.2 剪贴板格式读取

| 项目 | 说明 |
|------|------|
| **原实现** | `OpenClipboard` → `EnumClipboardFormats` → `GetClipboardData`，读取 CF_UNICODETEXT / CF_DIB / CF_DIBV5 / CF_HDROP / 自定义格式 |
| **关键细节** | 二进制 DIB 解析、CF_HDROP (DROPFILES 结构体) 解析、UTF-16 处理、OLE 回退 |
| **Rust 方案** | `clipboard-rs` 的 `get_text()` / `get_image()` / `get_files()` / `get_html()` |
| **难度** | ⭐⭐ 较易 |
| **备注** | 不需要手动 `EnumClipboardFormats`。`clipboard-rs` 提供 `has(ContentFormat::Text/Image/Files)` 检查格式存在性，在 `on_clipboard_change` 回调中按需读取即可。图片读写格式为 PNG（平台内部转换），需要 DIB 持久化时自行存/读原始字节。 |

### 1.3 剪贴板写入

| 项目 | 说明 |
|------|------|
| **原实现** | `WriteText` / `WriteImage` / `WriteFilePaths` — 向系统剪贴板写入数据 |
| **关键细节** | 自写入跟踪 (SelfWriteTracker) |
| **Rust 方案** | `clipboard-rs` 的 `set_text()` / `set_image()` / `set_files()` |
| **难度** | ⭐ 简单 |
| **备注** | 仅写入，**不做**原项目的 `PostMessage(WM_PASTE)` + `keybd_event(Ctrl+V)` 模拟粘贴。用户点击"粘贴"只负责写到剪贴板，不自动切换窗口粘贴。自写入跟踪用哈希+时间戳实现。 |

### 1.4 条目去重 (Deduplication)

| 项目 | 说明 |
|------|------|
| **原实现** | SHA-256 哈希（文本内容或图像像素）→ 若命中则 `UpsertDedup`（刷新 `updated_at` + upsert formats） |
| **关键细节** | 文本: trimmed 内容 SHA256；图像: 解码后取 NRGBA 像素哈希；CF_HDROP 特殊标记 `"hdrop:" + hash` |
| **Rust 方案** | `sha2` crate |
| **难度** | ⭐ 简单 |
| **备注** | 纯逻辑，无平台依赖。 |

### 1.5 来源追踪 (Source Tracking)

| 项目 | 说明 |
|------|------|
| **原实现** | `GetClipboardOwner` → `GetWindowThreadProcessId` → `QueryFullProcessImageName` + `GetWindowText` |
| **关键细节** | 记录 `source_exe` 和 `source_title`，WebView2 自写入覆盖为 "jPaste" |
| **Rust 方案** | `windows` crate P/Invoke |
| **难度** | ⭐⭐ 较易 |
| **备注** | 标准 Win32 调用。 |

### 1.6 标签分类 (Tag Mask)

| 项目 | 说明 |
|------|------|
| **原实现** | 位掩码：Text(1) / Image(4) / URL(8) / File(16) / Favorite(32)，在捕获时计算 |
| **关键细节** | URL 检测（`http(s)://` 开头）、Windows 路径检测（`X:\` / `\\`） |
| **Rust 方案** | 纯逻辑 |
| **难度** | ⭐ 简单 |
| **备注** | 无难度，直接移植。 |

---

## 2. UI 层功能

> ⚠️ 此部分重构工作量大。原项目前端是 React + TS（约 9503 个前端文件含依赖），egui 是即时模式 GUI，很多交互模式差异巨大。
>
> **框架选择**：不使用 eframe，改用 `egui_winit` + `egui_glow` 自行管理事件循环。原因：需要失焦隐藏窗口，eframe 的 `ViewportCommand::Visible(false)` 会导致事件循环停滞。winit 原生支持 `WindowEvent::Focused(false)` 和 `window.set_visible(false)`。

### 2.1 主窗口 / 历史列表

| 项目 | 说明 |
|------|------|
| **原实现** | React 组件：EntryItem (~320 行) + EntryList + 若干交互逻辑 |
| **关键细节** | 每条：序号图、内容预览、时间/来源、操作按钮组（收藏/编辑器/删除/复制/粘贴）。hover 显示按钮组，默认隐藏。无限滚动分页。 |
| **Rust 方案** | egui `ScrollArea` + 自定义 widget |
| **难度** | ⭐⭐⭐ 中等 |
| **备注** | Action 模块、图片查看器已删除，复杂度大幅降低。egui 实现：ScrollArea 包裹垂直布局，每条渲染 text + 按钮。缺虚拟列表（数据量大时可加）。可简化：复制动画、tooltip 浮动层、图片 hover 放大遮罩、正则搜索、文件路径复制按钮均可不做。 |

### 2.2 标签过滤 (Tab 栏)

| 项目 | 说明 |
|------|------|
| **原实现** | 顶部 Tab 栏：全部 / 文本 / 图片 / 网址 / 文件 / 收藏 |
| **关键细节** | 切换 → 重置分页 → 重新查询 |
| **Rust 方案** | egui `TopBottomPanel` + 自定义 tab 按钮 |
| **难度** | ⭐⭐ 较易 |
| **备注** | 简单的 tab 切换，无难度。 |

### 2.3 搜索栏

| 项目 | 说明 |
|------|------|
| **原实现** | 输入框 + 正则切换按钮 + 排序下拉 + 清空按钮 + 统计（总条数/当前筛选结果） |
| **关键细节** | 支持全文搜索和正则搜索，光标分页（20 条/页），支持按更新时间/内容长度排序（升/降序） |
| **Rust 方案** | egui `TextEdit` + `ComboBox` + 自定义按钮 |
| **难度** | ⭐⭐⭐ 中等 |
| **备注** | 搜索逻辑清晰，难点在于光标分页的状态维护。 |

### 2.4 条目操作按钮

| 项目 | 说明 |
|------|------|
| **原实现** | 主操作（复制/粘贴）始终显示；次要操作（编辑器打开、删除、收藏、文件路径复制）悬停时显示。分组按钮布局。 |
| **关键细节** | 悬停显隐、动画、快捷键数字指示器（Ctrl+1~9） |
| **Rust 方案** | egui 悬停检测（`response.hovered()`）+ 条件渲染 |
| **难度** | ⭐⭐⭐ 中等 |
| **备注** | 需要精细的 hover 状态管理，在 egui 中需要手动跟踪。 |

### 2.5 设置页面

| 项目 | 说明 |
|------|------|
| **原实现** | React 设置页：热键配置、保留天数、默认操作、开机自启、最小化启动、通知开关/透明度/预览、粘贴顺序、排序配置、清空全部、统计信息 |
| **关键细节** | 热键冲突检测、通知预览、Toast 透明度滑块 |
| **Rust 方案** | egui 普通页面，表单控件，配置项简化（因大部分功能已删除） |
| **难度** | ⭐ 简单 |
| **备注** | 需要在 egui 中自定义热键配置 UI（按键捕获），其余均为标准表单控件。设置数据存储为 `settings.json`（`serde_json`）。 |

### 2.6 主题系统

| 项目 | 说明 |
|------|------|
| **原实现** | 三套 CSS 主题（冷调极简 / 暖调高效 / 深色沉浸），CSS 变量驱动 |
| **关键细节** | 主题通过 `document.documentElement.className` 切换 |
| **Rust 方案** | 一套固定的「梦幻浅紫」主题，直接硬编码 egui `Style` 颜色值 |
| **难度** | ⭐ 简单 |
| **备注** | 不需要主题切换逻辑。直接定义一套紫色调的 egui 配色（主色、背景、文字、边框等），启动时设置 `ctx.set_style()` 即可。 |

### 2.7 Toast 通知

| 项目 | 说明 |
|------|------|
| **原实现** | 预创建隐藏无框窗口（360×80），离屏定位。WebView2 始终保持渲染，事件驱动显示/隐藏。底部右侧弹出，3s 后自动隐藏。 |
| **关键细节** | 独立窗口，不抢焦点 (`IgnoreMouseEvents`)、透明度可配置 |
| **Rust 方案** | egui `show_viewport_deferred` 创建独立窗口 + `ViewportCommand` 控制属性 |
| **难度** | ⭐⭐⭐ 中等 |
| **备注** | egui 原生多视口系统支持创建独立 OS 窗口。无框：`Decorations(false)`，置顶：`WindowLevel(AlwaysOnTop)`，不抢焦点：`MousePassthrough(true)`，透明：`Transparent(true)`。用 `Deferred` 模式使 Toast 窗口独立于主窗口渲染循环，主窗口隐藏时也能弹出。 |

### 2.8 系统托盘 (System Tray)

| 项目 | 说明 |
|------|------|
| **原实现** | Wails 内置系统托盘：图标 → 弹出菜单（显示 / 设置 / 退出）。点击托盘切换窗口显隐。 |
| **关键细节** | 图标嵌入 (`//go:embed logo.png`) |
| **Rust 方案** | `tray-icon` crate (v0.24.1, tauri 团队维护) |
| **难度** | ⭐⭐⭐ 中等 |
| **备注** | ✅ **支持**。`tray-icon` 在独立后台线程运行 Win32 消息泵，通过 `set_event_handler` + `EventLoopProxy` 将托盘事件注入主线程 winit 事件循环。配合 `egui_winit` 模式，托盘点击 → `UserEvent` → 主循环处理。 |

### 2.9 图片缩略图 (Lazy Loading)

| 项目 | 说明 |
|------|------|
| **原实现** | `useImageThumbnail` hook，IntersectionObserver 检测可见性，异步加载 PNG 缩略图 |
| **关键细节** | 缩略图缓存、Base64 Data URL 渲染 |
| **Rust 方案** | egui 支持 `Image` widget，需要异步加载图片文件 → 解码 → 上传为纹理 |
| **难度** | ⭐⭐⭐ 中等 |
| **备注** | egui 原生 `Image` 支持纹理。难点在于异步加载 + 纹理管理，防止主线程卡顿。 |

### 2.10 快捷键帮助弹窗

| 项目 | 说明 |
|------|------|
| **原实现** | `ShortcutHelpModal` 组件，`?` 键触发，显示所有可用快捷键 |
| **关键细节** | 全快捷键列表展示 |
| **Rust 方案** | egui `Window` modal |
| **难度** | ⭐ 简单 |
| **备注** | 无难度。 |

---

## 3. 保留的高级功能

### 3.1 文件操作 (编辑器打开 / Explorer 打开)

| 项目 | 说明 |
|------|------|
| **原实现** | `OpenInEditor`：写临时文件 → 尝试 VS Code 打开 → fallback 系统默认。`OpenInExplorer`：调用 `explorer.exe` |
| **关键细节** | 文件格式检测 (`detectFormat`)、临时文件清理 |
| **Rust 方案** | `std::process::Command` 调用外部程序 |
| **难度** | ⭐ 简单 |
| **备注** | 无平台依赖困难。 |

### 3.2 粘贴顺序控制 (FiloStack)

| 项目 | 说明 |
|------|------|
| **原实现** | WH_KEYBOARD_LL 全局键盘钩子拦截 Ctrl+V → 从内存队列中弹出内容 → 写入剪贴板 → 放行 Ctrl+V |
| **关键细节** | Strategy 模式（目前仅 QueueStrategy FIFO）、自写入保护、自粘贴保护、自动退出（检测到非文本时）|
| **Rust 方案** | `windows_hook` crate 的 `KeyboardLLHook` / `WindowsHookBuilder` 封装 `SetWindowsHookEx(WH_KEYBOARD_LL)` |
| **难度** | ⭐⭐⭐ 中等 |
| **备注** | ✅ **支持**。`windows_hook` crate (v0.1.3) 提供安全 Rust 封装：`WH::KEYBOARD_LL` + 自动生命周期管理。回调中解析 `KBDLLHOOKSTRUCT` 判断 Ctrl+V，弹出队列 → `clipboard-rs set_text()` → `CallNextHookEx` 放行。**注意**：WH_KEYBOARD_LL 需要消息泵线程，需单独线程运行 `GetMessage` 循环。 |

---

## ~~3. 已删除的功能~~

以下功能**确认不做**：

- **Action 模块系统**（原 12 个模块：math/json/url/folder/base64/unicode/curl/ws/timestamp/urldecode）
- **JSON 查看器**（独立窗口树形编辑器）
- **图片查看器**（独立窗口缩放拖拽）
- **Curl 调试器**（HTTP 请求编辑/发送）
- **WebSocket 调试器**（WS 连接/收发）
- **粘贴模拟**（原项目的 PostMessage + keybd_event 自动切换窗口粘贴）

---

## 4. 快捷键与热键

### 4.1 全局热键

| 项目 | 说明 |
|------|------|
| **原实现** | `Alt+V` 默认，`golang.design/x/hotkey` 库。可配置，冲突检测。 |
| **关键细节** | 配置变更时热键注册切换、冲突检测（返回中文错误） |
| **Rust 方案** | `global_hotkey` crate (v0.8.0, tauri 团队维护) |
| **难度** | ⭐⭐ 较易 |
| **备注** | ✅ **支持**。`GlobalHotKeyManager::new()` → `register(hotkey)`，在 `update()` 中 `GlobalHotKeyEvent::receiver().try_recv()` 轮询。dev-dependency 包含 `eframe 0.27`，官方验证兼容。 |

### 4.2 窗口内快捷键

| 项目 | 说明 |
|------|------|
| **原实现** | 大量快捷键：`Ctrl+L` 聚焦搜索 / `Ctrl+E` 编辑器打开 / `Ctrl+1~9` 执行 / `↑↓` 导航 / `Enter` 执行 / `Delete` 删除 / `Space` 收藏 / `Home/End` 滚动 / `PageUp/Down` 翻页 / `Esc` 清空搜索→隐藏 / `?` 帮助 / `F12` DevTools |
| **关键细节** | 焦点管理 + 键盘导航驱动的交互体系 |
| **Rust 方案** | egui 原生支持：`ctx.input().key_pressed(...)` / `ctx.input().modifiers.ctrl` / `ctx.input().key_down(...)` |
| **难度** | ⭐⭐ 较易 |
| **备注** | ✅ **原生支持**。在 `update()` 中直接判断按键状态。`ctx.input()` 提供完整键盘状态：`key_pressed`（按下瞬间）、`key_down`（持续按住）、`modifiers`（Ctrl/Alt/Shift/Win）。需要自行实现焦点状态管理（当前选中条目索引）。 |

---

## 5. 系统集成

### 5.1 开机自启

| 项目 | 说明 |
|------|------|
| **原实现** | Wails `app.Autostart.Enable()` — 注册到 Windows 启动项 |
| **关键细节** | 设置变更时动态开关 |
| **Rust 方案** | `windows` crate 写注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` |
| **难度** | ⭐ 简单 |
| **备注** | ⚠️ **无专用 crate**。直接用 `windows` crate 的 `RegSetValueExW` 设置/删除注册表键值，约 10 行代码。 |

### 5.2 单实例限制

| 项目 | 说明 |
|------|------|
| **原实现** | Wails `SingleInstance` 配置，`OnSecondInstanceLaunch` 回调激活已存在窗口 |
| **关键细节** | 命名互斥体/Mutex + IPC 通信 |
| **Rust 方案** | `windows` crate 的 `CreateMutexW` + `GetLastError` 检测 `ERROR_ALREADY_EXISTS` |
| **难度** | ⭐ 简单 |
| **备注** | ⚠️ **无专用 crate**。直接用 `windows` crate 创建命名互斥体，检测到已存在则退出并激活已有窗口（`FindWindowW` + `SetForegroundWindow`），约 10 行代码。 |

### 5.3 最小化到托盘 / 失去焦点隐藏

| 项目 | 说明 |
|------|------|
| **原实现** | 关闭按钮→隐藏到托盘；`WindowLostFocus` → 自动隐藏；pin 模式可禁用自动隐藏 |
| **关键细节** | 窗口位置管理（离屏隐藏避免 WebView2 闪烁，egui 无此问题） |
| **Rust 方案** | `egui_winit` + `egui_glow` 自行管理事件循环，winit 原生处理窗口显隐 |
| **难度** | ⭐⭐⭐ 中等 |
| **备注** | **不使用 eframe**（框架决策）。核心逻辑：`WindowEvent::Focused(false)` → `window.set_visible(false)` 隐藏；`UserEvent::TrayShow` → `window.set_visible(true)` 唤出。`egui_winit` 负责 winit 事件 → egui 输入的转换，`egui_glow` 负责 OpenGL 渲染。除 ~200 行事件循环样板外，egui UI 代码与 eframe 完全一致。 |

### 5.4 清理孤立 WebView2 进程

| 项目 | 说明 |
|------|------|
| **原实现** | `cleanupOrphanedWV2()` — 枚举进程，清理父进程已死亡的 msedgewebview2.exe |
| **关键细节** | Toolhelp32Snapshot |
| **Rust 方案** | 不需要。egui 没有 WebView2 依赖。 |
| **难度** | **不适用** ❌ 不需要 |
| **备注** | egui 是原生渲染，无 WebView2。 |

---

## 6. 存储层

### 6.1 SQLite 数据库

| 项目 | 说明 |
|------|------|
| **原实现** | `modernc.org/sqlite` (纯 Go SQLite)，表结构：`clipboard_entry` + `clipboard_format` |
| **关键细节** | 光标分页查询、全文搜索 LIKE、批量加载 formats、upsert 去重 |
| **Rust 方案** | `rusqlite` crate |
| **难度** | ⭐⭐ 较易 |
| **备注** | SQLite 操作简单，`rusqlite` 成熟。使用原项目相同的 schema 可保证数据兼容。 |

### 6.2 图片文件存储

| 项目 | 说明 |
|------|------|
| **原实现** | `%APPDATA%/jPaste/images/{YYYY-MM-DD}/{uuid}.png` + `{uuid}.dib`，日期目录分组 |
| **关键细节** | DIB → PNG 转换 / 读取 DIB 回写剪贴板 / 批量删除 |
| **Rust 方案** | `image` crate 处理 DIB→PNG |
| **难度** | ⭐⭐ 较易 |
| **备注** | 文件操作简单。需要 `image` crate 解码 BMP/DIB。 |

### 6.3 Settings 存储

| 项目 | 说明 |
|------|------|
| **原实现** | JSON 文件 `%APPDATA%/jPaste/settings.json`，读写加锁 |
| **关键细节** | 热键变更回调、Observer 模式通知其他服务 |
| **Rust 方案** | `serde_json` + `serde` 序列化 |
| **难度** | ⭐ 简单 |
| **备注** | 直接移植。 |

### 6.4 日志系统

| 项目 | 说明 |
|------|------|
| **原实现** | `internal/log` 包，结构化日志 (`slog`)，写入 `%APPDATA%/jPaste/jpaste.log` |
| **关键细节** | 前端日志通过 `Events.Emit('frontend-log', ...)` 回传后端 |
| **Rust 方案** | `log` + `env_logger` / `tracing` crate |
| **难度** | ⭐ 简单 |
| **备注** | 无难度。 |

### 6.5 定时清理 (Cleanup)

| 项目 | 说明 |
|------|------|
| **原实现** | 启动时 + 保留天数变更时执行。删除过期条目（收藏除外）+ 清理图片文件 + 清理空目录 + 清理 TEMP 文件 |
| **关键细节** | `DeleteByEntry` 删除对应图片 / `CleanEmptyDirs` 递归清理空日期目录 |
| **Rust 方案** | 纯逻辑 |
| **难度** | ⭐ 简单 |
| **备注** | 无难度。 |

### 6.6 统计信息

| 项目 | 说明 |
|------|------|
| **原实现** | `GetStats()` 返回条目总数 + 总字节数（含图片文件大小） |
| **Rust 方案** | SQL 查询 + 文件遍历 |
| **难度** | ⭐ 简单 |
| **备注** | 无难度。 |

---

## 7. 其他 / 工具类

### 7.1 自写入跟踪 (SelfWriteTracker)

| 项目 | 说明 |
|------|------|
| **原实现** | `util.SelfWriteTracker` — 记录最后写入的文本哈希 + 时间戳，避免自身写入被重新捕获后推送队列 |
| **Rust 方案** | `HashSet<String>` + 过期时间 |
| **难度** | ⭐ 简单 |
| **备注** | 简单逻辑。 |

### 7.2 工具函数

| 项目 | 说明 |
|------|------|
| **原实现** | SHA256、文本截断、DIB→BMP 头部拼接、`FormatInt` |
| **Rust 方案** | `sha2` crate 或内建 |
| **难度** | ⭐ 简单 |
| **备注** | 直接移植。 |

---

## 8. 总结

### 8.1 总体评估

| 类别 | 数量 | 难度分布 |
|------|------|----------|
| 核心基础 (1.x) | 6 | 简单×4, 较易×2 |
| UI 层 (2.x) | 9 | 简单×3, 较易×1, **中等×5**, 困难×1 |
| 保留高级功能 (3.x) | 2 | 简单×1, 中等×1 |
| 快捷键 (4.x) | 2 | 较易×2 |
| 系统集成 (5.x) | 3 | 简单×2, 中等×1 |
| 存储层 (6.x) | 6 | 简单×5, 较易×1 |
| 工具类 (7.x) | 2 | 简单×2 |
| **合计** | **30** | **简单 17 / 较易 8 / 中等 6 / 困难 1** |

### 8.2 功能清单（一次完成，不分优先级）

| 类别 | 功能 | crate/方案 | crates.io 可用 |
|------|------|------------|:---:|
| 核心 | 1.1 剪贴板监听 | `clipboard-rs` (ClipboardWatcherContext) | ✅ |
| 核心 | 1.2 格式读取 | `clipboard-rs` (get_text/image/files) | ✅ |
| 核心 | 1.3 写入 | `clipboard-rs` (set_text/image/files) | ✅ |
| 核心 | 1.4 去重 | `sha2` (SHA-256) | ✅ |
| 核心 | 1.5 来源追踪 | `windows` crate 自写 P/Invoke | ⚠️ 需自写 |
| 核心 | 1.6 标签分类 | 纯逻辑 | ✅ |
| UI | 2.1 主窗口列表 | egui `ScrollArea` + 自定义 widget | ✅ egui 内置 |
| UI | 2.2 标签过滤 | egui tab 按钮 | ✅ |
| UI | 2.3 搜索栏 | egui `TextEdit` + `ComboBox` | ✅ |
| UI | 2.4 操作按钮 | egui hover 检测 | ✅ |
| UI | 2.5 设置页面 | egui 表单 + `serde_json` settings | ✅ |
| UI | 2.6 主题 | egui `ctx.set_style()` 固定配色 | ✅ |
| UI | 2.7 Toast 通知 | egui `show_viewport_deferred` | ✅ |
| UI | 2.8 系统托盘 | `tray-icon` crate | ✅ |
| UI | 2.9 图片缩略图 | egui `Image` + 异步加载 | ✅ |
| UI | 2.10 快捷键帮助 | egui `Window` modal | ✅ |
| 高级 | 3.1 文件操作 | `std::process::Command` | ✅ |
| 高级 | 3.2 粘贴顺序控制 | `windows_hook` crate (WH_KEYBOARD_LL) | ✅ |
| 热键 | 4.1 全局热键 | `global_hotkey` crate | ✅ |
| 热键 | 4.2 窗口快捷键 | egui `ctx.input().key_pressed()` | ✅ egui 内置 |
| 系统 | 5.1 开机自启 | `windows` crate 写注册表 | ⚠️ 需自写 ~10 行 |
| 系统 | 5.2 单实例 | `windows` crate CreateMutexW | ⚠️ 需自写 ~10 行 |
| 系统 | 5.3 托盘/失焦隐藏 | `egui_winit` + `egui_glow` 自行管理事件循环 | ⚠️ 需自写 ~200 行样板 |
| 系统 | 5.4 清理 WebView2 | 不需要 | N/A |
| 存储 | 6.1 SQLite | `rusqlite` | ✅ |
| 存储 | 6.2 图片文件 | `image` crate | ✅ |
| 存储 | 6.3 Settings | `serde_json` + `serde` | ✅ |
| 存储 | 6.4 日志 | `log` + `env_logger`/`tracing` | ✅ |
| 存储 | 6.5 清理 | 纯逻辑 + 文件操作 | ✅ |
| 存储 | 6.6 统计 | SQL 查询 + 文件遍历 | ✅ |
| 工具 | 7.1 自写入跟踪 | 纯逻辑 | ✅ |
| 工具 | 7.2 工具函数 | `sha2` / 纯逻辑 | ✅ |

### 8.3 关键风险点

1. **UI 工作量仍最大**：egui 是即时模式 GUI，列表、布局、样式需从零构建。但 Action 模块和图片查看器已删除，估算主 UI 代码 ~580 行（事件循环样板 ~150 + 搜索栏 ~60 + 条目渲染 ~180 + 分页 ~50 + 缩略图 ~60 + 设置页 ~80），整体可控。

2. **框架选型已定：`egui_winit` + `egui_glow`**：放弃 eframe，自行管理事件循环。`egui_winit` 负责事件转换，`egui_glow` 负责渲染。需 ~200 行样板代码，但换来完全掌控窗口显隐和焦点行为。UI 层代码（`update()` 函数）写法与 eframe 一致。

3. **全局键盘钩子 (WH_KEYBOARD_LL)**：粘贴顺序控制需要独立线程+消息泵，与 egui 主循环需要协调。

4. **数据兼容性**：SQLite schema 可以直接复用，图片存储路径也可以保持一致，实现无缝迁移。

### 8.4 已确定取舍

| 功能 | 决定 | 理由 |
|------|------|------|
| 粘贴模拟 (keybd_event Ctrl+V) | **❌ 去掉** | 只写剪贴板，不做自动切窗口模拟粘贴 |
| 格式枚举 (EnumClipboardFormats) | **❌ 不需要** | `clipboard-rs` 的 `has(Text/Image/Files)` 可替代 |
| 来源追踪 | **✅ 保留** | 自写 ~40 行 P/Invoke 封装 |
| 主题切换 | **❌ 不需要** | 仅一套「梦幻浅紫」固定主题 |
| Toast 通知 | **✅ egui 多窗口实现** | `show_viewport_deferred` + `ViewportCommand` |
| Action 模块系统 | **❌ 删除** | 12 个模块全部不做 |
| JSON/图片/Curl/WS 查看器 | **❌ 删除** | 查看器型独立窗口全部不做 |
| 设置页 UI | **✅ 保留** | 功能精简，egui 表单页面 |
| 文件操作 | **✅ 保留** | `OpenInEditor` / `OpenInExplorer` |
| 粘贴顺序控制 | **✅ 保留** | `windows_hook` crate 支持 WH_KEYBOARD_LL |

**确认不做的功能**：

- Action 模块系统（math / json / url / folder / base64 / unicode / curl / ws / timestamp / urldecode）
- JSON 查看器（独立窗口树形编辑器）
- 图片查看器（独立窗口缩放拖拽）
- Curl 调试器（HTTP 请求编辑/发送）
- WebSocket 调试器（WS 连接/收发）

---

## 9. 依赖清单 (Cargo.toml)

### 9.1 主依赖

| Crate | 版本 | 用于 | 功能确认 |
|-------|------|------|:--------:|
| `egui` | `0.35` | 即时模式 GUI 核心库 | ✅ 所有 UI 组件原生支持 |
| `egui-winit` | `0.35` | winit 事件 → egui 输入转换 | ✅ 替代 eframe，配合 winit 事件循环 |
| `egui_glow` | `0.35` | OpenGL 渲染后端 | ✅ features = `["winit"]` |
| `winit` | `0.30` | 原生窗口创建 + 事件循环 | ✅ `Focused(false)`、`set_visible(true/false)`、`EventLoopProxy` 均支持 |
| `clipboard-rs` | `0.3.5` | 剪贴板读写 + 监听 | ✅ 文本/图片/文件读写、ClipboardWatcherContext 监听 |
| `rusqlite` | `0.40` | SQLite 数据库 | ✅ features = `["bundled"]` 自带 SQLite |
| `tray-icon` | `0.24.1` | 系统托盘图标 + 菜单 | ✅ Windows/Linux/macOS，`set_event_handler` + EventLoopProxy |
| `global_hotkey` | `0.8.0` | 全局热键 (Alt+V) | ✅ Windows/Linux/macOS，`try_recv()` 轮询 |
| `windows_hook` | `0.1.3` | WH_KEYBOARD_LL 键盘钩子 (FiloStack) | ✅ `WH::KEYBOARD_LL` + 自动生命周期管理 |
| `windows` | `0.62` | Win32 API 调用 | ✅ 来源追踪/开机自启/单实例 |
| `image` | `0.25` | 图片编解码 (DIB↔PNG) | ✅ features = `["bmp", "png"]` 支持 BMP/DIB 解码 |
| `serde` | `1` | 序列化框架 | ✅ features = `["derive"]` |
| `serde_json` | `1` | JSON 配置读写 | ✅ settings.json |
| `sha2` | `0.10` | SHA-256 哈希去重 | ✅ |
| `log` | `0.4` | 日志门面 | ✅ |
| `env_logger` | `0.11` | 日志后端（文件/终端） | ✅ 自动配置 |
| `uuid` | `1` | 图片文件 UUID 命名 | ✅ |

### 9.2 功能与 Crate 对照

```
┌────────────────────────────────────────────────────────────────────┐
│                       功能 → Crate 映射                            │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1.1 剪贴板监听    clipboard-rs (ClipboardWatcherContext)           │
│  1.2 格式读取      clipboard-rs (get_text/get_image/get_files)     │
│  1.3 写入          clipboard-rs (set_text/set_image/set_files)     │
│  1.4 去重          sha2 (SHA-256)                                  │
│  1.5 来源追踪      windows (GetClipboardOwner / QueryFull...)      │
│  1.6 标签分类      纯逻辑                                            │
│                                                                     │
│  2.1 主窗口列表    egui (ScrollArea + widgets)                     │
│  2.2 标签过滤      egui (自定义 buttons)                            │
│  2.3 搜索栏        egui (TextEdit + ComboBox)                      │
│  2.4 操作按钮      egui (Button + hover 检测)                      │
│  2.5 设置页面      egui + serde_json                               │
│  2.6 主题          egui (ctx.set_style)                            │
│  2.7 Toast 通知    egui (show_viewport_deferred + ViewportCommand) │
│  2.8 系统托盘      tray-icon (后台线程 Win32 消息泵)               │
│  2.9 图片缩略图    image (PNG 解码) + egui (Image widget)          │
│  2.10 快捷键帮助   egui (Window modal)                             │
│                                                                     │
│  3.1 文件操作      std::process::Command (无额外依赖)              │
│  3.2 粘贴顺序控制  windows_hook (WH_KEYBOARD_LL)                   │
│                                                                     │
│  4.1 全局热键      global_hotkey (GlobalHotKeyManager)             │
│  4.2 窗口快捷键    egui (ctx.input().key_pressed)                  │
│                                                                     │
│  5.1 开机自启      windows (RegSetValueExW)                        │
│  5.2 单实例        windows (CreateMutexW)                          │
│  5.3 托盘/失焦隐藏 winit (Focused + set_visible) + egui-winit     │
│                                                                     │
│  6.1 SQLite 存储   rusqlite                                        │
│  6.2 图片文件      image (PNG encode) + std::fs                   │
│  6.3 Settings      serde + serde_json                              │
│  6.4 日志          log + env_logger                                │
│  6.5 清理          rusqlite + std::fs                             │
│  6.6 统计          rusqlite + std::fs                              │
│                                                                     │
│  7.1 自写入跟踪    纯逻辑                                            │
│  7.2 工具函数      sha2 / 纯逻辑                                   │
│                                                                     │
└────────────────────────────────────────────────────────────────────┘
```

### 9.3 Cargo.toml 草案

```toml
[package]
name = "jpastev2"
version = "0.1.0"
edition = "2024"

[dependencies]
# GUI
egui = "0.35"
egui-winit = "0.35"
egui_glow = { version = "0.35", features = ["winit"] }
winit = "0.30"

# Clipboard
clipboard-rs = "0.3.5"

# Storage
rusqlite = { version = "0.40", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }

# Image
image = { version = "0.25", default-features = false, features = ["png", "bmp", "jpeg"] }

# System tray
tray-icon = "0.24"

# Global hotkey
global_hotkey = "0.8"

# Keyboard hook (filo stack)
windows_hook = "0.1"

# Win32 APIs (source tracking, autostart, single instance)
windows = "0.62"

# Hashing
sha2 = "0.10"

# Logging
log = "0.4"
env_logger = "0.11"

[target.'cfg(windows)'.dependencies]
# Windows-specific deps if needed
```

### 9.4 不依赖 eframe 的说明

本方案**不使用 `eframe`**，原因是：

| 问题 | 细节 |
|------|------|
| `eframe` 的 `ViewportCommand::Visible(false)` | 导致事件循环停滞，无法再次显示窗口 |
| 我们需要 | 失焦 `Focused(false)` → 完全隐藏 (`set_visible(false)`) |
| 解决方案 | `egui-winit` + `egui_glow` + `winit` 自行管理事件循环 |
| 额外代码量 | ~200 行事件循环样板（一次编写，后续不改） |
| UI 代码写法 | 与 eframe 完全相同（`ctx.run()` 内仍是 egui 代码） |
