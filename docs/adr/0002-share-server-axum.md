# ADR-0002: ShareServer 使用 axum 作为内嵌 HTTP 服务框架

- Status: Accepted (2026-07-11)
- Context: 工具箱新增 ShareServer（HTTP 共享服务器），需在 Tauri 应用内嵌一个监听 `0.0.0.0`、随机端口的 HTTP 服务，对外提供局域网文件下载 / 文本复制。其生命周期必须**绑定到共享面板窗口**（打开=启动，关闭=停止），且面板为单例（同一时刻只有一个服务）。Tauri v2 的异步运行时是 tokio。

## Decision

- 使用 **axum** 作为内嵌 HTTP 服务框架。
- 服务在共享面板窗口打开时经 `tauri::async_runtime::spawn` 启动；axum 的 `serve(...).with_graceful_shutdown(...)` 接收窗口 `destroyed` 事件触发的 `oneshot` 信号，实现「关闭面板=停止服务」。
- 面板使用**固定窗口标签** `share-panel`（不走 viewer 的每次唯一标签逻辑），保证单例与服务唯一。
- 网卡枚举引入 `get_if_addrs`；随机端口用 `std::net::TcpListener::bind("0.0.0.0:0")` 取操作系统分配端口后转为 tokio listener。

## Considered Options

- **axum（选定）**：tokio 原生，直接运行在 Tauri 已有 tokio 运行时上；`with_graceful_shutdown` 与窗口关闭事件天然契合；`Router` / `State` 提取器对「会话 + 共享状态」模型非常顺手；是当前事实上的现代标准。
- **warp**：同样 tokio 原生，但已进入维护态，社区动量转向 axum，不值得新项目引入。
- **actix-web**：依赖独立的 actix 运行时，与 Tauri 的 tokio 运行时冲突（需嵌套 actix System）→ 否决。
- **tiny_http**：同步、依赖极小，但需手动起线程 +  shutdown 标志，对共享状态、优雅关闭、路由都不如 axum 顺手 → 否决（仅在二进制体积极端敏感时才考虑）。

## Consequences

- + 服务生命周期与窗口天然绑定，正是本功能的核心范围决策（**刻意不做成脱离窗口的常驻守护进程**）。未来读者若以为「共享服务器应在窗口关闭后继续跑」，此 ADR 说明这是有意选择。
- + 复用 Tauri 的 tokio 运行时，无第二个异步运行时。
- - 新增 `axum` + 显式 `tokio` 依赖（tokio 本就由 Tauri 传递引入），桌面端二进制体积可接受。
- - 服务仅在面板打开期间运行（有意为之，见 `CONTEXT.md` 的 `ShareServer`）。
