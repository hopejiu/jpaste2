# ADR-0001: 统一全局快捷键注册表与强制互斥

- Status: Accepted (2026-07-09)
- Context: jPaste 已有全局快捷键（主窗口 `Alt+V`，`tauri_plugin_global_shortcut`）。合并 QuickLauncher 后，启动器目标也需全局热键。两者共用同一管理器，若各自注册会出现重复注册报错或行为歧义（同一键既弹窗又启动程序）。

## Decision

- 所有全局快捷键经单一 `build_shortcut_map(settings) -> HashMap<String, Action>` 构建，`Action` 枚举区分 `MainWindow` 与 `LaunchTarget(id)`。
- 注册/注销走差分（移植 quick-web-v2 `sync_hotkeys` 思想），只对变化的键操作。
- **强制互斥（项目铁律）**：任何两个全局快捷键不得相同，含主窗口键与启动器键之间。保存前 `validate` 给出冲突报错。
- `clipboard_enabled=false` 不注册主窗口键；`launcher_enabled=false` 不注册任何启动器键。
- 未来新增快捷键必须接入此注册表。

## Consequences

- + 无重复注册、无歧义行为；冲突在保存时被拦截。
- + 单一可信源，易审计、易测试。
- - 所有功能的热键逻辑集中，新增功能需遵循约定（已写入 `CONTEXT.md`）。
