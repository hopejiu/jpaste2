# jPaste Aggregator — PRD

> 在 jPaste v2 (Tauri + Preact) 基础上，从纯剪贴板工具升级为桌面聚合工具。

---

## 1. 产品定位

jPaste 是一款桌面聚合工具，以剪贴板管理为核心，平级附加"快速启动"和"工具箱"两个模块。Alt+V 全局唤出，用完即隐。

---

## 2. 导航模型

| 项 | 决策 |
|---|---|
| 窗口尺寸 | ~700×560（现有 480×560 放大）|
| 导航方式 | 左侧细侧边栏，Fluent Icon 按钮，单选切换 |
| 三个模块（从上到下） | 📋 剪贴板 / 🚀 快速启动 / 🧰 工具箱 |
| 默认着陆 | 剪贴板模块（`/`） |
| 路由结构 | `/` 剪贴板, `/quicklaunch` 快速启动, `/toolbox` 工具箱, `/settings` 设置（不变）|

Alt+V 呼出时：
- 窗口隐藏状态 → 显示窗口，默认回到剪贴板视图
- 窗口已显示 → 隐藏窗口（行为不变）

---

## 3. 剪贴板模块（现有，零变更）

### 3.1 保留功能
- 剪贴板历史捕获（文本/图片/文件）
- 标签过滤（全部/文本/图片/链接/文件/收藏）
- 搜索（全文 + 正则）
- 排序（时间/大小/复制次数）
- 复制/粘贴/编辑/删除/收藏
- 二维码检测
- FiloStack 粘贴模式（normal/stack/queue）
- Toast 通知 + Action Chip
- Viewer 窗口（JSON/Curl/WS/Calc/Decoder/Timestamp）
- 设置页（快捷键/保留天数/通知/自动收藏等全部保留）

### 3.2 变化
- **标题栏左侧新增 Home 按钮**（Fluent Icon "home"），点击进入快速启动模块
- **标题栏文字 "jPaste" 保持不变**

---

## 4. 快速启动模块

### 4.1 LaunchTarget 数据模型

```rust
pub struct LaunchTarget {
    pub id: String,               // UUID
    pub name: String,             // 显示名称
    pub kind: LaunchTargetKind,   // "web" | "file"
    pub target: String,           // URL 或 exe/lnk 绝对路径
    pub hotkey: Option<String>,   // "Alt+Shift+1" 格式，可选
    pub enabled: bool,
}

pub enum LaunchTargetKind {
    Web,   // http(s) URL
    File,  // .exe / .lnk 文件
}
```

存储位置：合并入 `settings.json`，`SettingsService.Data` 增加 `launch_targets: Vec<LaunchTarget>`。

### 4.2 UI 布局

- 顶部：标题栏（"快速启动"）+ 右侧 "+" 按钮
- 主体：垂直列表，每行一个 LaunchTarget
  - 左：kind icon（地球/web 或 文件/file）
  - 中：名称 + 副标题（URL 或文件路径截断显示）
  - 右：快捷键 badge（可点击录制）+ 启用开关 + 删除按钮
- 空状态："还没有启动目标，点击 + 添加"

### 4.3 添加/编辑 — 弹窗

点击 "+" 或行内编辑按钮弹出模态框：

**web 类型：**
- 名称（必填）
- URL（必填，自动 trim 首尾空格，自动补 `https://` 前缀）
- 快捷键（可选，复用现有 `HotkeyEditor` 组件监听键盘输入 + 冲突实时检测，不能等于 Alt+V）

**file 类型：**
- 名称（必填）
- 文件路径（必填，Tauri dialog 选文件，过滤器 `*.exe;*.lnk`）
- 快捷键（可选，同上）

保存后实时更新列表，同步更新全局快捷键注册。

### 4.4 触发方式

| 方式 | 行为 |
|---|---|
| 列表点击 | 立即执行 |
| 全局快捷键 | 立即执行（web 创建窗口 / file spawn） |
| 剪贴板 Toast Action Chip | 不接入（剪贴板内容不关联启动） |

### 4.5 Web 窗口生命周期

```
快捷键/点击 → toggle WebView 窗口
  ├─ 窗口不存在 → 创建 + 显示
  ├─ 窗口可见且聚焦 → 隐藏（走 1min 失焦隐藏定时器）
  ├─ 窗口可见但不聚焦 → 置顶 + 聚焦
  ├─ 窗口已隐藏 → 显示 + 聚焦（取消 destroy 定时器）
  │
  ├─ 用户手动 × 关闭 → 标记 closing，真实 destroy
  ├─ 最小化/失焦 1min → 自动 hide
  │   └─ hide 后 10min 内无唤出 → destroy
  └─ 保持在用 → 常驻

web 窗口无数量上限。
```

- 窗口标题 = LaunchTarget.name
- 无地址栏/工具栏/导航按钮
- 无 JS 脚本注入（简化，不移植 quick-web-v2 的 script_injector）

### 4.6 File 启动

- `kind="file"` → `std::process::Command::new(path).spawn()`
- 一次性启动，不追踪 PID，不做窗口唤出
- 不统计到 web 窗口上限

---

## 5. 工具箱模块

### 5.1 布局

网格卡片布局（如 3 列），每张卡片：
- Fluent Icon（约 28px）
- 名称（12-13px）
- 点击 → 调用 `open_viewer(route, -1)` 打开 viewer 窗口（传入 `id=-1` 表示工具箱触发的空白页面，非剪贴板条目）

### 5.2 初始插件清单

| 名称 | route | Icon | 说明 |
|------|-------|------|------|
| JSON 查看 | `/viewer/json` | Code | 空白 JSON 编辑器，可粘贴/格式化/树查看 |
| HTTP 调试 | `/viewer/curl` | Globe | 空白 Curl 调试器 |
| WS 调试 | `/viewer/ws` | Chat | 空白 WebSocket 调试器 |
| 计算器 | `/viewer/calc` | Calculator | 空白计算器 |
| 解码工具 | `/viewer/decoder` | Lock | 空白解码器（base64/URL/Unicode） |
| 时间戳转换 | `/viewer/timestamp` | Clock | 空白时间戳转换器 |

所有 viewer 窗口为独立 Tauri 无框窗口（复用现有 `open_viewer` 命令）。

---

## 6. 全局快捷键

### 6.1 铁律

```
所有快捷键在同一注册表内强制互斥。
```

### 6.2 注册表

| 键 | 所有者 | 是否可改 | 条件 |
|---|---|---|---|
| `Alt+V` | 剪贴板主窗口 | ❌ 不可改，始终生效 | 始终注册 |
| `Alt+Shift+N` | QuickLaunch target N | ✅ 用户可改 | target.enabled == true |

### 6.3 校验规则

保存前 validate：
1. 不能等于 `Alt+V`
2. 不能与其它 target 快捷键相同
3. 不能与系统已占用快捷键冲突（临时注册 + 注销验证）

### 6.4 注册策略

差分注册/注销（借鉴 quick-web-v2 的 `sync_hotkeys`）：
- 计算新旧注册表差集
- 只注销 removed 的，只注册 added 的
- 交集不动

---

## 7. 数据存储

### 7.1 文件结构

```
%APPDATA%/jpaste2/
  ├─ jpaste.db          # SQLite，剪贴板历史（不变）
  ├─ images/            # 图片文件（不变）
  └─ settings.json      # 设置 + QuickLaunch targets（不变）
```

### 7.2 settings.json 新增字段

```json
{
  "...": "(现有字段不变)",
  "launch_targets": [
    {
      "id": "a1b2c3d4",
      "name": "GitHub",
      "kind": "web",
      "target": "https://github.com",
      "hotkey": "Alt+Shift+G",
      "enabled": true
    },
    {
      "id": "e5f6g7h8",
      "name": "Notepad++",
      "kind": "file",
      "target": "C:\\Program Files\\Notepad++\\notepad++.exe",
      "hotkey": null,
      "enabled": true
    }
  ]
}
```

---

## 8. 设置页

不变。仅保留剪贴板专属设置：
- 全局快捷键（Alt+V，只读展示不可改）
- 保留天数
- 通知开关
- 粘贴模式
- 排序
- 自动收藏
- 开机自启 / 启动最小化
- 居中显示 / 复制后自动隐藏

---

## 9. Rust 后端变更清单

### 9.1 新增文件

无。所有变更在现有文件内完成。

### 9.2 变更文件

| 文件 | 变更 |
|---|---|
| `service/settings.rs` | `Data` 增加 `launch_targets: Vec<LaunchTarget>` + `LaunchTarget` 结构体 + `LaunchTargetKind` 枚举 |
| `command/mod.rs` | 新增 `LaunchTarget` 相关的 commands 注册 |
| `command/quicklaunch.rs` | **新建** — `get_launch_targets`, `save_launch_targets`, `launch_target`, `check_target_hotkey` |
| `command/viewer.rs` | `open_viewer` 处理 `id=-1` 表示工具箱空白页（跳过数据加载）|
| `lib.rs` | `setup_hotkeys` 扩展为多键注册（Alt+V + QuickLaunch 目标键），添加差分 sync 逻辑 |
| `lib.rs` | `build_services` 初始化阶段从 settings 读取 launch_targets 并注册快捷键 |

### 9.3 新增 command

```rust
#[tauri::command]
fn get_launch_targets(state) -> Vec<LaunchTarget>

#[tauri::command]
fn save_launch_targets(app, state, targets: Vec<LaunchTarget>) -> Result<(), SaveError>

#[tauri::command]
fn launch_target(app, state, id: String)

#[tauri::command]
fn check_target_hotkey(app, state, hotkey_str: String, editing_id: Option<String>) -> Result<(), String>

#[tauri::command]
fn pick_file_path() -> Option<String>   // 原生文件选择器，过滤 exe/lnk
```

---

## 10. 前端变更清单

### 10.1 新增文件

| 文件 | 说明 |
|---|---|
| `src/routes/quicklaunch/index.tsx` | 快速启动模块主视图 |
| `src/routes/quicklaunch/launch-modal.tsx` | 添加/编辑启动目标的弹窗 |
| `src/routes/quicklaunch/launch-modal.tsx` | 添加/编辑启动目标的弹窗（复用 `HotkeyEditor` 组件）|
| `src/routes/toolbox/index.tsx` | 工具箱模块：网格卡片列表 |

### 10.2 变更文件

| 文件 | 变更 |
|---|---|
| `src/app.tsx` | 增加 `/quicklaunch` 和 `/toolbox` 路由 |
| `src/routes/main/index.tsx` | 标题栏增加侧边栏切换按钮（三个图标），窗口尺寸放宽适配 |
| `src/lib/invoke.ts` | 增加新的 API 绑定 |
| `src/lib/types.ts` | 增加 `LaunchTarget` / `LaunchTargetKind` 类型定义 |

### 10.3 Viewer 文件变更

所有 viewer 页面（`json-view.tsx`、`curl-view.tsx`、`ws-view.tsx` 等）：
- 现有逻辑：通过 `?id=N` 加载剪贴板条目数据
- 新增路径：当 `id` 解析为 `-1`（或缺失）时，展示空白初始状态，让用户自由输入

---

## 11. 交互原型示意

```
┌──────────────────────────────────────────────────────┐
│ [📋] [🚀] [🧰]                    jPaste    [🔧] [📌] │  ← 标题栏
├──────────────────────────────────────────────────────┤
│                                                      │
│  (当前模块内容区)                                    │
│                                                      │
│                                                      │
└──────────────────────────────────────────────────────┘

剪贴板模块（📋）:
┌──────────────────────────────────────────────────────┐
│ [📋] [🚀] [🧰]                    jPaste    [🔧] [📌] │
├─ 全部 │ 文本 │ 图片 │ 链接 .................... [.*] │
│ [搜索框................................................]│
│ ┌─────────────────────────────────────────────────┐  │
│ │ 📄 Hello World                     2026-07-09  │  │
│ │ 🔗 https://example.com             2026-07-09  │  │
│ │ 🖼️ image.png                       2026-07-09  │  │
│ │ 📁 C:\Users\...                    2026-07-09  │  │
│ └─────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────┘

快速启动模块（🚀）:
┌──────────────────────────────────────────────────────┐
│ [📋] [🚀] [🧰]                    jPaste    [🔧] [📌] │
│ 快速启动                                      [＋]   │
│ ┌─────────────────────────────────────────────────┐  │
│ │ 🌐 GitHub          Alt+Shift+G  [🔘 on] [✏️] [🗑️]│  │
│ │ 🌐 DeepSeek        Alt+Shift+D  [🔘 on] [✏️] [🗑️]│  │
│ │ 📁 Notepad++                    [🔘 on] [✏️] [🗑️]│  │
│ └─────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────┘

工具箱模块（🧰）:
┌──────────────────────────────────────────────────────┐
│ [📋] [🚀] [🧰]                    jPaste    [🔧] [📌] │
│ 工具箱                                               │
│ ┌──────────┐ ┌──────────┐ ┌──────────┐             │
│ │   { }    │ │   🌐    │ │   💬    │             │
│ │ JSON 查看 │ │ HTTP调试 │ │ WS调试   │             │
│ ├──────────┤ ├──────────┤ ├──────────┤             │
│ │   🧮    │ │   🔐    │ │   🕐    │             │
│ │  计算器  │ │ 解码工具 │ │时间戳转换│             │
│ └──────────┘ └──────────┘ └──────────┘             │
└──────────────────────────────────────────────────────┘
```

---

## 12. 不做的事（明确排除）

| 功能 | 理由 |
|---|---|
| 快速启动分组/文件夹 | YAGNI，列表已够用 |
| 全局搜索 | 剪贴板搜索 + 快速启动搜索不需要合并 |
| Web 窗口脚本注入 | 复杂度 > 收益，quick-web-v2 特化需求 |
| 启动后常驻快速启动窗口 | file 一次性，web 有生命周期管理 |
| 工具箱插件市场/热加载 | 只有 6 个固定插件，硬编码即可 |
| 主题/换肤 | 保持现有 Fluent UI 风格不变 |
| 移动端 | Tauri desktop only |
