# jPaste UI/UX 重构 Checklist

> 设计目标：WinUI 3 / Fluent UI 风格，淡紫色，Win10+Win11 兼容，高效剪贴板管理器

---

## 设计系统决策

| 决策项 | 选择 |
|--------|------|
| 字体 | Segoe UI Variable Text → Segoe UI → Microsoft YaHei → system-ui |
| 主色 | #6264E7（WinUI 默认紫） |
| 圆角 | 4/8/12/16px（WinUI 3 四级） |
| 阴影 | 0/4/8/16 四级 |
| 间距 | 严格 4px 网格 |
| 图标 | Fluent UI SVG Icons |
| 动效 | 完整 WinUI 3 动效（Toast 除外） |
| 可访问性 | 全量修复 |

---

## Phase 1: 设计系统（CSS 变量） ✅

- [x] 1.1 颜色令牌重写（Primary #6264E7 + 中性色 + 语义色）
- [x] 1.2 字体栈（Segoe UI Variable + 微软雅黑回退）
- [x] 1.3 圆角变量（4/8/12/16px）
- [x] 1.4 阴影变量（0/4/8/16 级）
- [x] 1.5 间距变量（4/8/12/16/24/32px）
- [x] 1.6 动效变量（duration + easing）
- [x] 1.7 Focus Ring 样式
- [x] 1.8 Reduced Motion 媒体查询

## Phase 2: 图标系统 ✅

- [x] 2.1 安装 @fluentui/svg-icons
- [x] 2.2 创建 FluentIcon wrapper 组件（硬编码路径数据）
- [x] 2.3 替换 entry-item.tsx 图标
- [x] 2.4 替换 main/index.tsx 图标
- [x] 2.5 替换 settings/index.tsx 图标
- [x] 2.6 替换 action-module-list.tsx 图标
- [x] 2.7 替换 modal.tsx / toast.tsx / viewer 图标
- [x] 2.8 移除 lucide-preact 依赖

## Phase 3: 主页面 ✅

- [x] 3.1 TitleBar 加高到 48px + 应用图标
- [x] 3.2 搜索栏样式优化
- [x] 3.3 Tag Tabs 样式优化
- [x] 3.4 EntryItem 布局重构（2行内容 + 40px按钮）
- [x] 3.5 EntryList 间距优化
- [x] 3.6 底部栏样式优化
- [x] 3.7 快捷键帮助模态框样式

## Phase 4: 设置页面 ✅

- [x] 4.1 设置页 Header 样式
- [x] 4.2 分组卡片样式
- [x] 4.3 Toggle Switch WinUI 化
- [x] 4.4 Hotkey Editor 样式
- [x] 4.5 Action Module List 样式
- [x] 4.6 清空确认模态框样式

## Phase 5: 可访问性 ✅

- [x] 5.1 全局 Focus Ring 样式
- [x] 5.2 所有 icon 按钮添加 aria-label
- [x] 5.3 Modal 焦点陷阱
- [x] 5.4 prefers-reduced-motion 支持

## Phase 6: 动效 ✅

- [x] 6.1 按钮按压动效（scale + opacity）
- [x] 6.2 列表项聚焦背景渐变
- [x] 6.3 模态框打开/关闭动效
- [x] 6.4 操作成功反馈动效
- [x] 6.5 删除条目滑出动效

## Phase 7: 其他页面 ✅

- [x] 7.1 Toast 页面样式（无动画）
- [x] 7.2 Viewer 页面样式统一

---

## 已完成

- 2026-07-03：Phase 1-7 全部完成
  - CSS 设计系统全面重写（WinUI 3 风格）
  - Lucide → Fluent UI SVG 图标迁移
  - 列表项 2行 + 40px 按钮布局
  - 48px TitleBar + 应用图标
  - 设置页分组卡片化
  - Modal 焦点陷阱 + 可访问性
  - 完整 WinUI 3 动效系统
  - TypeScript 编译通过
