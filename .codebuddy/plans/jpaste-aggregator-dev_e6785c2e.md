---
name: jpaste-aggregator-dev
overview: 基于 PRD 将 jPaste 从纯剪贴板工具升级为聚合工具。Rust 后端新增 command/quicklaunch.rs 模块、settings.rs 增加 LaunchTarget 数据、viewer.rs 支持 id=-1、lib.rs 快捷键扩展。前端新建 4 个文件（QuickLaunch 模块、添加弹窗、快捷键录制器、工具箱网格）+ 修改现有 5 个文件（路由、主页面侧边栏、types、invoke、tauri.conf.json 窗口尺寸）。
todos:
  - id: backend-data-model
    content: 在 settings.rs 添加 LaunchTarget/Kind 结构体及 Data.launch_targets 字段；新建 command/quicklaunch.rs 实现 5 个命令；在 command/mod.rs 注册
    status: completed
  - id: backend-hotkey-viewer
    content: 扩展 lib.rs：多快捷键差分注册/注销 + handler 分发；修改 viewer.rs open_viewer 支持 id=-1 空白页
    status: completed
    dependencies:
      - backend-data-model
  - id: frontend-types-api-config
    content: 添加前端 LaunchTarget/LaunchTargetKind 类型定义；invoke.ts 加 5 个 API 绑定；tauri.conf.json width 480→700
    status: completed
    dependencies:
      - backend-data-model
  - id: frontend-layout-routing
    content: 重构 app.tsx 路由（+/quicklaunch +/toolbox）；标题栏改为侧边栏导航+激活态高亮；global.css 侧边栏布局样式
    status: completed
    dependencies:
      - frontend-types-api-config
  - id: quicklaunch-ui
    content: 实现快速启动模块：hotkey-recorder.tsx 快捷键捕获组件 + launch-modal.tsx 弹窗 + index.tsx 列表视图
    status: completed
    dependencies:
      - frontend-layout-routing
  - id: toolbox-ui-viewer-blank
    content: 实现工具箱网格卡片页面；修改 6 个 viewer 页面支持 id=-1 空白初始态
    status: completed
    dependencies:
      - frontend-layout-routing
---

## 核心需求

将 jPaste 从纯剪贴板工具升级为聚合工具，三个平级模块通过左侧侧边栏切换。

### 功能模块

1. **剪贴板模块**（已有，零变更）：保留全部现有功能，Alt+V 始终有效不可更改
2. **快速启动模块**（新增）：管理 LaunchTarget（web URL 开 WebView 窗口 / file exe/lnk 一次性运行），自带行内编辑+弹窗，支持全局快捷键扩展
3. **工具箱模块**（新增）：网格卡片展示 6 个插件入口（JSON/HTTP/WS/计算器/解码/时间戳），点击开 viewer 窗口（空白页）

### 交互细节

- 窗口 480x560 → 700x560，左侧 44px 窄侧边栏三个 Fluent Icon 按钮切换
- LaunchTarget 添加/编辑用模态弹窗，web 类型填名称+URL，file 类型用原生文件选择器
- URL 输入自动优化：trim 首尾空格，若不含协议前缀自动补充 `https://`
- 快捷键设定方式：复用现有 `HotkeyEditor` 组件（`src/components/hotkey-editor.tsx`），监听键盘组合键输入，跟剪贴板设置页一致
- Web 窗口快捷键为 toggle 行为：
- 窗口不存在 → 创建 + 显示
- 窗口可见且聚焦 → 隐藏（走 1min 失焦隐藏定时器）
- 窗口可见但不聚焦 → 置顶 + 聚焦
- 窗口已隐藏 → 显示 + 聚焦（取消 destroy 定时器）
- Web 窗口生命周期：失焦 1min 自动隐藏，隐藏后 10min 不唤出则 destroy，无上限
- 快捷键同一注册表强制互斥，差分注册/注销

### 数据存储

- launch_targets 合并入现有 settings.json，SettingsService.Data 增加字段

### 不做

- 二级菜单、分组管理、脚本注入、主题换肤、移动端

## Tech Stack

- **前端**: Preact + TypeScript + wouter (hash router) + Fluent SVG Icons
- **后端**: Rust + Tauri v2 + tauri-plugin-global-shortcut + tauri-plugin-opener
- **存储**: serde_json + settings.json (现有 SettingsService)
- **构建**: Vite + pnpm

## Architecture Design

### Component Architecture

```
[App.tsx - Router + Sidebar Layout]
  ├── [路由 "/"]          → MainPage (剪贴板，不变)
  ├── [路由 "/quicklaunch"] → QuickLaunchPage (快速启动)
  ├── [路由 "/toolbox"]     → ToolboxPage (工具箱)
  ├── [路由 "/settings"]    → SettingsPage (不变)
  └── [路由 "/viewer/*"]    → Viewer pages (id=-1 兼容空白态)
```

### Data Flow

```
Frontend (Preact)                    Backend (Rust)
===============                      ==============
QuickLaunchPage ── invoke ──→  command::quicklaunch ──→ SettingsService.Data.launch_targets
  │                                    │
  │  api.openViewer(route, -1)         └── lib.rs: setup_hotkeys → global_shortcut plugin
  │       │                                    │
  │       └──→ command::viewer::open_viewer    └── global_shortcut handler → launch_target
  │                │
  │                └──→ WebviewWindowBuilder → viewer window
  │
ToolboxPage ── api.openViewer(route, -1) ──→ command::viewer::open_viewer
```

### Module Division

| Layer | Module | Files |
| --- | --- | --- |
| Data Model | service/settings | LaunchTarget, LaunchTargetKind, Data.launch_targets |
| Commands | command/quicklaunch | 5 Tauri commands |
| Commands | command/viewer | id=-1 blank state support |
| Backend Wiring | lib | Multi-hotkey registration, diff sync, dispatch |
| Frontend API | lib/invoke.ts | Typed invoke wrappers |
| Frontend Types | lib/types.ts | LaunchTarget interface |
| QuickLaunch UI | routes/quicklaunch/ | List, Modal (复用 HotkeyEditor) |
| Toolbox UI | routes/toolbox/ | Grid cards |
| Viewer UI | routes/viewer/* | Blank initial state for id=-1 |


## Implementation Notes

### Performance

- Web 窗口无数量上限，每个 web 窗口独立管理生命周期
- 快捷键差分注册/注销 O(n) 复杂度，n 为 launch_targets 数量（通常 < 50），无需优化
- viewer 空白页无数据加载，零性能开销

### Logging

- 复用现有 `log::info!/debug!` 模式
- quicklaunch 命令执行加 info 日志：`"quicklaunch: executing {} ({})", name, kind`
- 快捷键触发加 debug 日志：`"hotkey: dispatch {} → {} (enabled={})"`
- 定期清理定时器加 debug 日志
- 避免记录文件路径全貌（可能含用户敏感目录）

### Blast Radius

- MainPage 标题栏重构：保持 `data-tauri-drag-region` 不变，只改内部按钮布局
- SettingsPage 零改动
- viewer 页面向后兼容：id=-1 走新路径，id>0 走现有加载路径
- toast/create_toast_window_inner 等不相关代码零改动
- 窗口尺寸放宽：minWidth 保持 360 不变，width 改为 700（不影响功能，仅视觉）

## Directory Structure

```
src-tauri/
├── src/
│   ├── command/
│   │   ├── mod.rs                 # [MODIFY] +pub mod quicklaunch, 注册命令
│   │   ├── quicklaunch.rs         # [NEW] 5 commands: get/save/launch/check/pick
│   │   └── viewer.rs              # [MODIFY] id=-1 blank state
│   ├── service/
│   │   └── settings.rs            # [MODIFY] LaunchTarget/Kind structs, Data.launch_targets
│   └── lib.rs                     # [MODIFY] multi-hotkey, diff sync, handler dispatch
src/
├── lib/
│   ├── types.ts                   # [MODIFY] LaunchTarget, LaunchTargetKind
│   └── invoke.ts                  # [MODIFY] api.getLaunchTargets, save, launch, check, pick
├── routes/
│   ├── main/
│   │   └── index.tsx              # [MODIFY] sidebar nav buttons + layout
│   ├── quicklaunch/
│   │   ├── index.tsx              # [NEW] list view
│   │   └── launch-modal.tsx       # [NEW] add/edit modal (复用 HotkeyEditor)
│   ├── toolbox/
│   │   └── index.tsx              # [NEW] grid cards
│   ├── viewer/
│   │   ├── json-view.tsx          # [MODIFY] id=-1 blank state
│   │   ├── curl-view.tsx          # [MODIFY] id=-1 blank state
│   │   ├── ws-view.tsx            # [MODIFY] id=-1 blank state
│   │   ├── calc-view.tsx          # [MODIFY] id=-1 blank state
│   │   ├── decoder-view.tsx       # [MODIFY] id=-1 blank state
│   │   └── timestamp-view.tsx     # [MODIFY] id=-1 blank state
├── styles/
│   └── global.css                 # [MODIFY] sidebar styles
├── app.tsx                        # [MODIFY] +/quicklaunch +/toolbox routes
└── tauri.conf.json                # [MODIFY] window width 480→700
```

## Key Code Structures

### Rust — LaunchTarget data model (service/settings.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchTargetKind {
    Web,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchTarget {
    pub id: String,
    pub name: String,
    pub kind: LaunchTargetKind,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool { true }
```

### TypeScript — Frontend types (lib/types.ts)

```typescript
export type LaunchTargetKind = 'web' | 'file';

export interface LaunchTarget {
  id: string;
  name: string;
  kind: LaunchTargetKind;
  target: string;
  hotkey: string | null;
  enabled: boolean;
}
```

## Agent Extensions

### MCP

- **go**: Not used — this is a Rust/Tauri project, not a Go project

### Skills

- **design-to-code-workflows**: Not needed — no Figma/screenshot input, using existing Fluent UI patterns
- **improve-codebase-architecture**: Not needed in this plan (architecture review already done, this is a feature addition following existing patterns)