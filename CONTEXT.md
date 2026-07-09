# jPaste

jPaste 是一款系统级剪贴板增强工具，监听剪贴板变化，保存历史记录，通过 Tauri 无框窗口提供 toast 通知和 viewer 窗口。

## Language

**Action（动作/插件）**:
对剪贴板内容类型的自动检测和对应操作。每个 Action 有 `detect()`（内容是否匹配）和 `handler()`（匹配后的执行逻辑）。Action 的检测代码在 Rust 和 TypeScript 两侧独立存在：Rust 侧用于 toast 增强（约 30 行），TypeScript 侧用于主窗口列表渲染（复用 `useMemo` 零开销）。
_Avoid_: 插件，module

**Toast**:
捕获剪贴板后在右下角短暂出现的小窗口（340px 宽，高度动态 70-130px）。纯提示性，3 秒后自动隐藏，不获取焦点。当检测到 Action 匹配时，Toast 下方显示可点击的 Action chip（圆角药丸按钮，左 icon 右文字）。当捕获的图片包含二维码时，Toast 显示"复制二维码"chip，点击直接复制二维码内容。

**Action Chip**:
Toast 底部横排的交互式按钮，左 icon 右文字标签。默认显示当前匹配的 Action，点击直接执行该 Action（不弹二级确认）。

**Viewer**:
独立的 Tauri 无框窗口，展示某条剪贴板条目内容的详细视图（JSON 树、Curl 调试器、WS 调试器、计算器、解码器、时间戳转换器）。

**Entry**:
剪贴板历史中的一条记录，包含内容文本、哈希、标签位掩码、图片路径、收藏状态、二维码内容等。

**Tag Mask**:
位掩码标记条目类型（TAG_TEXT=1, TAG_IMAGE=4, TAG_URL=8, TAG_FILE=16, TAG_FAVORITE=32）。Rust 侧 `compute_tag_mask()` 计算，同一条目可多标签叠加。

## Aggregator features (added 2026-07-09)

jPaste 从纯剪贴板工具升级为聚合工具。主窗口尺寸放大到 ~700×560，左侧细侧边栏（Fluent Icon 按钮），右侧内容区。三个平级模块通过底部/侧边栏切换，无二级菜单。

**窗口导航**:
Alt+V（默认）呼出主窗口。左侧侧边栏三个图标按钮（从上到下）：剪贴板 / 快速启动 / 工具箱。点击切换右侧视图。剪贴板（`/`）是默认着陆页。**剪贴板模块不可关闭**，始终可通过唤出键或托盘（左键单击）打开。唤出键可在设置中更改或清空；清空后仅托盘可打开主窗口。

**QuickLauncher（快速启动）**:
聚合工具中的"快速启动"功能模块（源于 quick-web-v2 的站点启动能力）。每个启动目标是一个 LaunchTarget。模块自带行内编辑（CRUD），不依赖设置页。

**LaunchTarget（启动目标）**:
快速启动的一条配置。字段：`id`、`name`、`kind`（"web" | "file"）、`target`（URL 或文件绝对路径）、`hotkey: Option<String>`、`enabled`。
- `kind="web"` → 在应用内开 WebView 窗口（仅 http(s)）。快捷键/列表点击为 toggle 行为（创建/隐藏/显示），手动 × 关闭则 destroy。窗口生命周期：失焦 1min 后自动隐藏 → 隐藏后 10min 不唤出则 destroy。web 窗口无数量上限。
- `kind="file"`（仅 `.exe` / `.lnk`）→ `opener::open` 一次性启动，不追踪进程。

**Toolbox（工具箱）**:
网格卡片布局，每张卡片一个 Fluent Icon + 名称，点击打开对应的 viewer 窗口（复用 `open_viewer` 命令，传 id=-1 表示工具箱空白页）。初始含 6 个插件：JSON 查看、HTTP 调试 (Curl)、WS 调试、计算器、解码工具（base64/URL/Unicode）、时间戳转换。

**CurlViewer（HTTP 调试器）**:
Toolbox 中的 HTTP 请求调试器，以独立 viewer 窗口呈现。解析剪贴板里的 curl 命令为可编辑请求（方法 / URL / 头 / 体），发送后展示响应。响应头**保留同名多值**（多个 `set-cookie` 等不合并）；响应体按内容类型**自动以 JSON 树/代码视图或原样文本展示**，并允许手动覆盖视图模式。

**Global Shortcut（全局快捷键）**:
所有全局快捷键（主窗口唤出键、QuickLauncher 目标键）在**同一注册表**内**强制互斥**。主窗口唤出键可更改或清空；清空后仅托盘（左键单击）可打开主窗口。QuickLauncher 目标键保存/编辑前校验：
1. 不能与其它 QuickLauncher 目标键冲突
2. 不能与系统已占用快捷键冲突（临时注册验证）

保存时差分注册/注销。

