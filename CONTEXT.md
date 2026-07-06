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

