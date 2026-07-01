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

---

## 13. 工具箱新增：ShareServer（HTTP 共享服务器）

> 设计已与领域模型对齐，术语见 `CONTEXT.md` 的 `ShareServer`，架构决策见 `ADR-0002`。

### 13.1 领域定位（已对齐）

- 工具箱的一张卡片 → 打开「共享面板」viewer（**单例**，固定窗口标签 `share-panel`）。
- 服务生命周期**绑定面板窗口**：打开=启动，关闭（销毁）=停止。viewer 失焦不自动隐藏，故无「隐藏即退服」中间态。
- **单向（主机 → 局域网）**：客户端只能下载文件、复制文本到自己剪贴板，不可回传内容到主机。

### 13.2 已对齐设计决策

| 项 | 决策 |
|---|---|
| 共享模型 | 一个**会话** = 一份可增删的混合条目列表（文件可下载 / 文本可复制），支持多条 |
| 主机添加 | 文件选择对话框 + 拖拽 + 文本粘贴/输入；**不做**「分享当前剪贴板」；文件先复制到临时共享目录 |
| 访问控制 | **无口令**；监听 `0.0.0.0`；面板仅枚举物理网卡 IPv4（过滤 VPN/Docker/WSL/Hyper-V 虚拟网卡），每个 IP 一条独立 URL |
| 端口 | 每次随机空闲端口（OS 分配） |
| 呈现 | 每条 URL 配文字 + 复制按钮，二维码默认折叠、点击展开（复用现有 `generateQr` 命令） |
| 生命周期 | 条目仅主机手动删除；窗口关闭清会话 + 临时目录 |

### 13.3 后端实现（Rust）

新增 `src-tauri/src/command/share_server.rs`：

- **新增依赖**：`axum`、`tokio`（features 含 `rt-multi-thread`/`net`/`sync`/`signal`）、`get_if_addrs`（网卡枚举）。`tempfile` 已存在。
- **状态**：`Arc<Mutex<ShareState>>`，含 `session: Option<ShareSession>`；`ShareSession { items: Vec<ShareItem>, temp_dir: tempfile::TempDir, port: u16, shutdown_tx: Option<oneshot::Sender<()>> }`。
- **`ShareItem`**：`{ id: String, kind: "file"|"text", name: String, size: u64, payload: PathBuf|String }`。
- **命令**：
  - `start_share_server(app, state) -> Vec<ShareUrl>`：已运行则返回现有 URL；否则随机端口 bind `0.0.0.0`，枚举物理网卡 IPv4 拼 URL，spawn axum 服务（graceful shutdown 接 `shutdown_tx`）。
  - `stop_share_server(state)`：发 `shutdown_tx`、drop `temp_dir`、清空 session。
  - `add_share_file(state, path) -> ShareItem`：复制到 `temp_dir` 后登记。
  - `add_share_text(state, name, text) -> ShareItem`。
  - `remove_share_item(state, id)`、`list_share_items(state) -> Vec<ShareItem>`、`get_share_urls(state) -> Vec<ShareUrl>`（`ShareUrl { ip, port, url }`）。
- **axum 路由**：`GET /` 返回条目列表 HTML（文件→下载链接，文本→内容 + 复制按钮）；`GET /d/{id}` 流式下载文件（`Content-Disposition: attachment`）；`GET /t/{id}` 返回纯文本。HTML 内联最小 CSS，无前端框架。
- **网卡枚举**：遍历 `get_if_addrs::get_if_addrs()`，过滤 `is_loopback`、非 IPv4、以及虚拟网卡（按名称/类型排除 VPN/Docker/WSL/Hyper-V 的 vEthernet）。

### 13.4 窗口生命周期集成（关键）

- 共享面板使用**固定窗口标签** `share-panel`，不走 `open_blank_viewer` 的每次唯一标签逻辑。在打开逻辑中：标签已存在则聚焦，不新建、不重启服务。
- `setup` 中注册窗口事件：监听 `share-panel` 的 `destroyed` → 调用 `stop_share_server` 清理，落实「关闭 = 退服」。

### 13.5 前端实现

- 新增 `src/routes/share/index.tsx`，注册路由 `/share`。
- `src/routes/toolbox/index.tsx` 的 `TOOLS` 增加：`{ name:'HTTP 共享', icon:'globe', action:'viewer', route:'/share' }`；点击走单例逻辑（固定标签 `share-panel`）。
- 面板 UI：
  - 顶部风险条：「局域网内任何设备可访问，公共 WiFi 慎用」。
  - URL 区：`api.get_share_urls()` 列出每条（IP + 端口）文字 + 复制按钮；点「显示二维码」调 `api.generateQr({ content: url })` 渲染（复用现有 `generateQr`）。
  - 添加区：① 文件选择（Tauri dialog）→ `add_share_file`；② 拖拽区 → drop 拿路径批量添加；③ 文本输入 + 「添加文本」→ `add_share_text`。
  - 条目列表：`api.list_share_items()` 渲染，每条带「删除」→ `remove_share_item`。
- 开关：面板打开时调 `start_share_server`（拿 URL 渲染）；关闭由窗口事件自动停服，前端无需显式停止。

### 13.6 数据流时序

1. 点工具箱「HTTP 共享」→ 前端打开单例窗口 `share-panel`。
2. 面板 `onMount` → `start_share_server` → Rust 随机端口 + 枚举物理网卡 + spawn axum + 返回 URL 列表。
3. 添加文件/文本 → `add_share_*` → Rust 复制到 `temp_dir`、登记条目。
4. 局域网设备扫/输 URL → axum 返回列表页 → 下载文件 / 浏览器 `navigator.clipboard` 复制文本。
5. 关闭面板 → `destroyed` 事件 → `stop_share_server` → shutdown + 删 `temp_dir`。

### 13.7 测试要点

- 单例：连点卡片不启多服务；URL 一致。
- 关闭面板后端口释放、`temp_dir` 删除。
- 多网卡：仅物理网卡出 URL；VPN/WSL/Docker 不出。
- 无口令访问：同网段另一设备可下载。
- 源文件删除后分享仍可用（已复制到 temp）。

