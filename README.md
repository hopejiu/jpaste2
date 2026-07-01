# jPaste v2

Windows 剪贴板管理器，使用 Rust + egui 构建。

## 截图

*（暂无）*

## 功能

| # | 功能 | 状态 |
|---|------|------|
| 1 | 剪贴板事件监听（clipboard-rs） | ✅ |
| 2 | 文本/图片/文件读写 | ✅ |
| 3 | SHA256 内容去重 | ✅ |
| 4 | 来源追踪（exe + 窗口标题） | ✅ |
| 5 | Tag 标签分类（文本/图片/网址/文件） | ✅ |
| 6 | 自写入跟踪（5s TTL） | ✅ |
| 7 | 历史列表 + 无限滚动 | ✅ |
| 8 | 全文搜索 + 排序 | ✅ |
| 9 | 标签过滤 Tab（6 个） | ✅ |
| 10 | 复制/收藏/删除/编辑器打开 | ✅ |
| 11 | Alt+方向键快捷键 | ✅ |
| 12 | FiloStack 队列粘贴模式（WH_KEYBOARD_LL） | ✅ |
| 13 | 设置页（热键/保留天数/通知/排序） | ✅ |
| 14 | 全局热键 Alt+V | ✅ |
| 15 | 系统托盘 | ✅ |
| 16 | Toast 通知（Win32 原生分层窗口） | ✅ |
| 17 | 梦幻浅紫主题 | ✅ |
| 18 | 单实例限制 | ✅ |
| 19 | 定时清理 + 收藏豁免 | ✅ |
| 20 | 集成测试（35 场景） | ✅ |

## 技术栈

| 层 | 选型 |
|---|------|
| GUI 框架 | `egui` 0.35（即时模式） |
| 窗口管理 | `winit` 0.30 + `egui-winit` |
| 渲染后端 | `egui-wgpu`（D3D12 / Vulkan / Metal） |
| 剪贴板 | `clipboard-rs` |
| 数据库 | `rusqlite`（SQLite, WAL 模式） |
| 系统托盘 | `tray-icon` |
| 全局热键 | `global-hotkey` |
| Win32 API | `windows` 0.62（来源追踪/开机自启/单实例/Toast） |
| 图片 | `image` crate（PNG 解码） |
| 序列化 | `serde` + `serde_json` |
| 哈希 | `sha2` |

## 构建

```powershell
# 运行
cargo run

# 测试
cargo test

# 发布构建
cargo build --release
```

**系统要求**：Windows 10+，支持 DirectX 12 / Vulkan 的 GPU。

## 数据目录

`%APPDATA%/jPastev2/`

```
%APPDATA%/jPastev2/
├── clipboard.db        # SQLite 数据库
├── images/
│   └── YYYY-MM-DD/
│       └── {uuid}.png  # 剪贴板图片
├── settings.json        # 用户配置
└── jpaste.log           # 日志
```

## 快捷键

| 快捷键 | 功能 |
|--------|------|
| `↑ / ↓` | 移动条目焦点 |
| `Enter` | 复制焦点条目 |
| `Delete` | 删除焦点条目 |
| `Space` | 切换收藏 |
| `Home / End` | 滚动顶部 / 底部 |
| `PageUp / PageDown` | 翻页 |
| `Esc` | 清空搜索 / 隐藏窗口 |
| `?` | 快捷键帮助弹窗 |
| `Alt+V` | 全局显隐窗口 |

## 项目结构

```
src/
├── main.rs               # 入口
├── lib.rs                # 模块声明
├── app.rs                # 应用编排 + UI
├── ops.rs                # 条目操作函数
├── clipboard/            # 剪贴板监听 + 处理
├── filostack/            # 队列粘贴模式
├── settings/             # 配置管理
├── storage/              # SQLite + 文件存储
├── ui/                   # egui 界面
└── util/                 # 工具函数
tests/                    # 集成测试（35 场景）
```

## 架构设计

详见 [`docs/技术方案.md`](docs/技术方案.md)。

### 关键 seam

- **ClipboardService**：`handle(data, &repo, &image_store) → Result<Option<i64>>` — 剪贴板处理管道，可独立测试
- **ops 模块**：`copy_entry`、`delete_entry`、`toggle_favorite` 等 — 条目 CRUD，不依赖 App 结构体

## 许可

MIT
