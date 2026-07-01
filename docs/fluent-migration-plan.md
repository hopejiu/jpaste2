# Fluent UI Web Components 迁移计划

## 评估结论

### JSON Viewer 页面
- **依赖**: `jsoneditor` 第三方包（有 tree/code 两种模式、搜索、历史、导航栏等功能）
- **集成方式**: `use-json-editor` hook 通过动态 import 加载 jsoneditor 及其 CSS，占据全屏容器
- **与 Fluent 关系**: jsoneditor 是独立的全功能编辑器，管理自己的 DOM 和样式，与 Fluent 组件无直接冲突
- **结论**: **保持现状，不做改动**。jsoneditor 的功能无法被 Fluent 组件替代，强行替换会丢失大量既有功能。只需移除 `viewer-page` 等 CSS 类依赖，改用 inline styles 即可

---

## 迁移步骤

### Phase 1: 依赖安装与基础配置

#### 1.1 安装 Fluent UI Web Components
- [ ] `pnpm add @fluentui/web-components` — 安装 Fluent Web Components 包
- [ ] 确认 `@fluentui/svg-icons` 已在依赖中（当前已安装 `^1.1.331`）

#### 1.2 注册 Fluent Web Components
- [ ] 修改 `src/main.tsx`：
  - [ ] 添加 `import { defineFluentComponents } from '@fluentui/web-components';`
  - [ ] 在 `render(<App />, root)` 之前调用 `defineFluentComponents();`
  - [ ] 导入并包裹 `<fluent-design-system-provider>`，配置主题色：
    ```tsx
    <fluent-design-system-provider
      accent-base-color="#6264E7"
      base-layer-luminance={0.95}
      style={{ height: '100%' }}
    >
      <App />
    </fluent-design-system-provider>
    ```

#### 1.3 配置 Tauri 支持透明窗口（亚克力前置条件）
- [ ] 在 `src-tauri/tauri.conf.json` 中为所有窗口添加 `"transparent": true`（亚克力效果需要窗口背景透明）
- [ ] 确认 Rust 侧初始化窗口时没有设置不透明的背景色
- [ ] 如果 Tauri v2 使用 `SetBackgroundColor` API，确保设置为透明

#### 1.4 更新 TypeScript 类型
- [ ] 如果 tsconfig 开启了 `noUnusedLocals`，确保 Web Components 的 JSX 类型兼容
- [ ] 添加 Fluent 组件的事件类型映射（fluent-button 的 `click` 事件、fluent-switch 的 `change` 事件等）

---

### Phase 2: 样式系统迁移

#### 2.1 改造 `src/styles/global.css`
- [ ] **保留的 CSS**（Fluent 不提供的布局逻辑）：
  - `* { margin: 0; padding: 0; box-sizing: border-box; }`
  - `body` 基础样式（font-family、overflow、user-select）
  - `#app` 布局（`height: 100vh; display: flex; flex-direction: column`）
  - `.main-page`、`.title-bar`、`.entry-list`、`.entry-item`、`.entry-body` 等自定义布局类
  - `.bottom-bar`、`.queue-popup`、`.shortcut-groups` 等自定义组件样式
  - `::-webkit-scrollbar` 滚动条样式
  - `.json-*` JSON viewer 样式
  - `.curl-*`、`.ws-*`、`.calc-*`、`.decoder-*`、`.ts-*` 等 viewer 内部样式
  - `@media (prefers-reduced-motion: reduce)` 辅助功能样式

- [ ] **移除的 CSS**（由 Fluent Design System Provider 替代）：
  - `:root` 中的 `--color-*` 变量（Fluent DS Provider 会提供设计令牌）
  - `--font-family`、`--font-mono`（Fluent 管理排版）
  - `--radius-*`（Fluent 的 `corner-radius` 设计令牌）
  - `--shadow-*`（Fluent 的 elevation 系统）
  - `--focus-ring`、`--focus-ring-offset`（Fluent 焦点环）
  - `--duration-*`、`--easing-*`（Fluent 动效令牌）

- [ ] **调整颜色引用**：将文件中硬编码的 `var(--color-*)` 改为 Fluent 设计令牌

#### 2.2 配置 Design System Provider
- [ ] 在 `main.tsx` 的 `<fluent-design-system-provider>` 上设置：
  - `accent-base-color="#6264E7"` — 紫色主题主色
  - `base-layer-luminance={0.95}` — 浅色主题
  - `corner-radius={4}` — 统一圆角
  - `stroke-width={1}` — 边框粗细
  - 补充 CSS 变量覆盖：`--neutral-fill-rest`、`--neutral-fill-hover` 等

#### 2.3 添加亚克力（Acrylic）效果

##### 原理说明
亚克力是 Fluent Design System 的标志性材质，通过在背景上叠加模糊和色调实现毛玻璃效果。Web 上使用 CSS `backdrop-filter` + 半透明背景模拟，与 `@fluentui/web-components` 组件无冲突，因为组件的 Shadow DOM 内部会透传 `backdrop-filter`。

##### 实施步骤

- [ ] **创建 Acrylic CSS 工具类**，添加到 `global.css`：
  ```css
  /* ── Acrylic (亚克力) 毛玻璃效果 ────────────────────────────────── */
  .acrylic {
    background: rgba(255, 255, 255, 0.5);
    backdrop-filter: blur(30px) brightness(110%);
    -webkit-backdrop-filter: blur(30px) brightness(110%);
  }

  .acrylic-thick {
    background: rgba(255, 255, 255, 0.7);
    backdrop-filter: blur(50px) brightness(105%);
    -webkit-backdrop-filter: blur(50px) brightness(105%);
  }

  .acrylic-darker {
    background: rgba(32, 32, 32, 0.65);
    backdrop-filter: blur(30px) brightness(90%);
    -webkit-backdrop-filter: blur(30px) brightness(90%);
  }
  ```

- [ ] **标题栏应用亚克力** — `.title-bar` 添加 `.acrylic` 类，替换实色背景
- [ ] **搜索头/标签栏** — `.search-header` 和 `.tag-tabs-bar` 添加 `.acrylic` 类
- [ ] **底栏** — `.bottom-bar` 添加 `.acrylic` 类
- [ ] **队列弹出层** — `.queue-popup` 添加 `.acrylic-thick` 类（弹出层需要更厚的不透明度，保证内容可读性）
- [ ] **Modal 弹窗** — `.modal-content`（迁移后为 fluent-dialog 的内容区）添加 `.acrylic-thick` 类
- [ ] **SettingsPage 卡片** — 保持 `<fluent-card>` 实色背景（大面积卡片亚克力会导致性能下降）
- [ ] **Viewer 页面的标题栏** — 各 viewer 的 `.viewer-toolbar` 添加 `.acrylic` 类
- [ ] **Toast 窗口** — `.toast-card` 添加 `.acrylic-thick` 类

##### 注意事项
- Tauri 窗口默认不支持 `backdrop-filter`——需要在 `tauri.conf.json` 中启用：
  ```json
  {
    "windows": [
      {
        "transparent": true,
        "decorations": false
      }
    ]
  }
  ```
  或在 `main.tsx`/`main_helpers.rs` 中设置窗口透明（验证 Tauri v2 的 `SetBackgroundColor` API 是否兼容 `backdrop-filter`）
- 如果 Tauri 渲染器（WebView2）不支持 `backdrop-filter`，降级为半透明实色背景：
  ```css
  .acrylic {
    background: rgba(255, 255, 255, 0.85);
    @supports (backdrop-filter: blur(1px)) {
      background: rgba(255, 255, 255, 0.5);
      backdrop-filter: blur(30px) brightness(110%);
    }
  }
  ```
- 性能敏感区域（长列表、频繁重绘）避免使用亚克力
- `::-webkit-scrollbar` 轨道颜色需要适配半透明背景

---

### Phase 3: 公共组件替换

#### 3.1 Modal → `<fluent-dialog>`
- [ ] 修改 `src/components/modal.tsx`：
  - 使用 `<fluent-dialog>` 替换 `.modal-overlay` + `.modal-content`
  - fluent-dialog 内置焦点陷阱和 Escape 关闭
  - 保持 `children` 插槽
  - 添加 `<fluent-button appearance="stealth">` 做关闭按钮
- [ ] 更新所有调用方（ShortcutHelp、SettingsPage 的 ClearModal、ErrorAlert）

#### 3.2 ToggleSwitch → `<fluent-switch>`
- [ ] 修改 `src/components/toggle-switch.tsx`：
  - 将自定义 `<button role="switch">` 替换为 `<fluent-switch>`
  - 绑定 `checked` 属性到 `fluent-switch`
  - 监听 `change` 事件
- [ ] 更新调用方（SettingsPage、ActionModuleList）

#### 3.3 按钮迁移
- [ ] 将项目中所有 `<button class="viewer-btn primary">` 替换为 `<fluent-button appearance="accent">`
- [ ] 将项目中所有 `<button class="viewer-btn danger">` 替换为 `<fluent-button appearance="outline" style="--accent-fill-rest: #D13438">`
- [ ] 将项目中所有普通 `<button class="viewer-btn">` 替换为 `<fluent-button appearance="outline">`
- [ ] 将 `.title-btn` 替换为 `<fluent-button appearance="stealth">`
- [ ] `.act-btn` 保持在自定义 CSS 中保留（图标化按钮，Fluent 无直接等效）
- [ ] `.tag-tab` 保持在自定义 CSS 中保留（标签切换按钮）
- [ ] `.mode-btn` 保持在自定义 CSS 中保留（底部模式切换）

#### 3.4 输入框迁移
- [ ] `<input type="text">` → `<fluent-text-field>`
  - 注意：fluent-text-field 需要设置 `placeholder`、`value`（用 `:value` 绑定）、监听 `input` 事件
  - 搜索输入框、URL 输入框、表达式输入框等
- [ ] `<textarea>` → `<fluent-text-area>`
  - 请求体、编解码输入输出等
- [ ] `<select>` → `<fluent-select>` + `<fluent-option>`
  - 排序选择框、HTTP method 选择、Scheme 选择等

#### 3.5 其他 Fluent 组件替换
- [ ] `<input type="range">` → `<fluent-slider>`
  - SettingsPage 保留天数滑块
- [ ] `<input type="checkbox">` → `<fluent-checkbox>`
  - CurlViewPage 的跟随重定向选项
- [ ] 加载状态 `<div class="viewer-loading">` → `<fluent-progress-ring>`
- [ ] 卡片容器（`.viewer-section`、`.settings-section`）→ `<fluent-card>`

#### 3.6 无需替换的组件
- `<FluentIcon>` — 保持现有 SVG 图标组件（来自 `@fluentui/svg-icons`）
- `EntryItem` / `EntryList` — 自定义列表实现，保持现有结构，只更新内部按钮/开关
- `QueuePopup` — 自定义弹出层，无直接 Fluent 等效
- `HotkeyEditor` — 自定义快捷键编辑器，内部按钮可迁移
- `ActionModuleList` — 自定义模块列表，内部 ToggleSwitch/按钮可迁移
- `SectionHeader`（CurlViewPage 中的折叠卡片）— 考虑是否用 `<fluent-accordion>` 替代
- `KVTable`（CurlViewPage 中的键值表）— 自定义表格，保持

---

### Phase 4: 页面迁移 — MainPage

#### 4.1 `src/routes/main/index.tsx`
- [ ] 引入 Fluent 组件替换：
  - `.title-btn` → `<fluent-button appearance="stealth" size="small">`
  - `.regex-toggle` → `<fluent-button appearance="outline">`
  - `.tag-tab` 保留自定义（无直接 Fluent 等效标签栏）
  - `.copy-all-btn` → `<fluent-button appearance="outline">`
  - `.help-btn` → `<fluent-button appearance="stealth" size="small">`
- [ ] `.search-header` 中的 input → `<fluent-text-field>`
- [ ] 排序 select → `<fluent-select>` + `<fluent-option>`
- [ ] Error Alert 中的确定按钮 → `<fluent-button appearance="accent">`
- [ ] 更新 css class 引用，移除被 Fluent 替代的样式

#### 4.2 `src/routes/main/search-bar.tsx`
- [ ] 替换 `<input class="search-input">` → `<fluent-text-field appearance="outline">`
- [ ] 替换 `<select class="sort-select">` → `<fluent-select>`
- [ ] 修复 ref 传递方式（fluent-text-field 通过 ref 获取内部 input）

#### 4.3 `src/routes/main/entry-item.tsx`
- [ ] 保持整体结构不变，只替换内部元素：
  - `.act-btn` 保持自定义（有特殊 hover/active 状态）
  - `.entry-item` 保持自定义（有聚焦指示条、悬停效果）
- [ ] 确保 `.entry-actions` 的显隐逻辑保留
- [ ] 确保 `ACTION_ICONS` 映射保留

#### 4.4 `src/routes/main/entry-list.tsx`
- [ ] 无直接 Fluent 替换，保持现有实现
- [ ] `.empty-state` 可保持自定义

#### 4.5 其他 MainPage 子组件
- [ ] `src/components/queue-popup.tsx` — 保持自定义
- [ ] `src/components/shortcut-help.tsx` — Modal 已经被 fluent-dialog 替换
- [ ] `src/hooks/use-main-shortcuts.ts` — 逻辑不变

---

### Phase 5: 页面迁移 — SettingsPage

#### 5.1 `src/routes/settings/index.tsx`
- [ ] `.settings-header` 中的返回按钮 → `<fluent-button appearance="stealth">`
- [ ] `.settings-section` → `<fluent-card>`
- [ ] `.settings-segment` 按钮 → `<fluent-button appearance="outline">` + active 用 accent
- [ ] `.settings-slider-row input[type="range"]` → `<fluent-slider>`
- [ ] `.settings-clear-btn` → `<fluent-button appearance="outline" style="--accent-fill-rest: #D13438">`
- [ ] `.viewer-btn`（预览通知按钮）→ `<fluent-button appearance="outline">`
- [ ] `ToggleSwitch` → `<fluent-switch>`
- [ ] 清空确认 Modal → `<fluent-dialog>` + `<fluent-button appearance="accent">`
- [ ] `.settings-saved` 徽标 → `<fluent-badge>`（可选）

#### 5.2 `src/components/hotkey-editor.tsx`
- [ ] 修饰键按钮 → `<fluent-button appearance="outline">`
- [ ] `.settings-key-input` → `<fluent-text-field>`

#### 5.3 `src/components/action-module-list.tsx`
- [ ] `ToggleSwitch` → `<fluent-switch>`
- [ ] 上移/下移/展开按钮 → `<fluent-button appearance="stealth">`
- [ ] `.action-module-item` → `<fluent-card>`

---

### Phase 6: 页面迁移 — Viewer 页面

#### 6.1 JSON Viewer（推荐保持现状）
- [ ] 移除 `class="viewer-page"` 引用，改用 inline style `height: 100vh; position: relative`
- [ ] 确保 jsoneditor 不会与 Fluent 样式冲突（jsoneditor CSS 在 Shadow DOM 外独立运行）
- [ ] 加载/错误状态使用 `<fluent-progress-ring>` 改善外观
- [ ] `use-json-editor.ts` — 无变化

#### 6.2 Image Viewer `src/routes/viewer/image-view.tsx`
- [ ] `.viewer-toolbar` 中的按钮 → `<fluent-button appearance="stealth">`
- [ ] 重置缩放按钮 → `<fluent-button appearance="outline">`
- [ ] 导航按钮（prev/next）保持自定义（圆形按钮，Fluent 无等效）
- [ ] `.image-container` 保持自定义（缩放/拖拽行为）
- [ ] 移除 `.viewer-page`、`.viewer-toolbar`、`.viewer-btn` CSS 类依赖

#### 6.3 Curl Viewer `src/routes/viewer/curl-view.tsx`
- [ ] 工具栏按钮 → `<fluent-button appearance="outline">`
- [ ] 发送按钮 → `<fluent-button appearance="accent">`
- [ ] Method select → `<fluent-select>` + `<fluent-option>`
- [ ] Scheme select → `<fluent-select>` + `<fluent-option>`
- [ ] URL input → `<fluent-text-field>`（需设置 `appearance="outline"`）
- [ ] body textarea → `<fluent-text-area>`
- [ ] 超时 input → `<fluent-number-field>`
- [ ] 跟随重定向 checkbox → `<fluent-checkbox>`
- [ ] curl-card → `<fluent-card>`
- [ ] `SectionHeader` → 考虑用 `<fluent-accordion>` 替代，或保持自定义
- [ ] `KVTable` 中的 inputs → `<fluent-text-field>`
- [ ] 加载状态 → `<fluent-progress-ring>`
- [ ] 复制按钮文案保留

#### 6.4 WS Viewer `src/routes/viewer/ws-view.tsx`
- [ ] 工具栏按钮 → `<fluent-button appearance="stealth">`
- [ ] 清空按钮 → `<fluent-button appearance="outline">`
- [ ] 连接/断开按钮 → `<fluent-button appearance="accent">` / `<fluent-button appearance="outline" style="--accent-fill-rest: #D13438">`
- [ ] URL input → `<fluent-text-field>`
- [ ] 消息 input → `<fluent-text-field>`
- [ ] 发送按钮 → `<fluent-button appearance="accent">`
- [ ] `.viewer-section` → `<fluent-card>`
- [ ] 消息日志保持自定义（有颜色区分 sent/received/system）
- [ ] `.ws-act-btn` → `<fluent-button appearance="outline" size="small">`

#### 6.5 Calculator Viewer `src/routes/viewer/calc-view.tsx`
- [ ] 工具栏保持
- [ ] 计算按钮 → `<fluent-button appearance="accent">`
- [ ] 表达式 input → `<fluent-text-field>`
- [ ] 键盘按键 → `<fluent-button appearance="outline">`
- [ ] 运算符按键 → `<fluent-button appearance="outline">` + accent color tint
- [ ] 清除按键 → `<fluent-button appearance="outline" style="--accent-fill-rest: #D13438">`
- [ ] `.viewer-section` → `<fluent-card>`
- [ ] 计算结果显示区保持自定义

#### 6.6 Decoder Viewer `src/routes/viewer/decoder-view.tsx`
- [ ] 工具栏按钮 → `<fluent-button appearance="stealth">`
- [ ] 编解码模式标签 → `<fluent-tabs>` + `<fluent-tab>`（替换自定义 `.decoder-tabs`）
- [ ] 反转/复制按钮 → `<fluent-button appearance="outline">`
- [ ] 编码/解码切换按钮 → `<fluent-button appearance="outline">`
- [ ] input textarea → `<fluent-text-area>`
- [ ] output textarea → `<fluent-text-area readonly>`
- [ ] `.viewer-section` → `<fluent-card>`

#### 6.7 Timestamp Viewer `src/routes/viewer/timestamp-view.tsx`
- [ ] 工具栏按钮 → `<fluent-button appearance="stealth">`
- [ ] 现在按钮 → `<fluent-button appearance="outline">`
- [ ] 时间戳 input → `<fluent-text-field>`
- [ ] 结果行中的复制按钮 → `<fluent-button appearance="stealth">`
- [ ] `.viewer-section` → `<fluent-card>`
- [ ] `.ts-row` 结果行保持自定义
- [ ] `.ts-error` 错误显示保持自定义

---

### Phase 7: 页面迁移 — ToastPage

#### 7.1 `src/routes/toast.tsx`
- [ ] Toast 卡片 → `<fluent-card>`
- [ ] FluentIcon 保留
- [ ] 布局保持自定义（align-items: flex-end 等）
- [ ] 不需要按钮替换（toast 无交互按钮）

---

### Phase 8: 清理工作

#### 8.1 移除已覆盖的自定义 CSS
- [ ] 清理 `global.css` 中被 Fluent Design System Provider 替代的样式
- [ ] 移除 `:root` 中的 `--color-*`、`--font-*`、`--radius-*`、`--shadow-*`、`--focus-ring-*`、`--duration-*`、`--easing-*`
- [ ] 清理 `.viewer-btn`、`.viewer-btn.primary`、`.viewer-btn.danger` 等被 Fluent Button 替代的类
- [ ] 清理 `.toggle-switch` 自定义开关样式（被 `<fluent-switch>` 替代）
- [ ] 清理 `.modal-*` 样式（被 `<fluent-dialog>` 替代）

#### 8.2 验证各组件集成
- [ ] 确保 fluent-button 在 Preact 中的事件绑定正常（使用 `onClick` 而非 `onclick`）
- [ ] 确保 fluent-switch 的 `checked` 属性和 `change` 事件在 Preact 中正常工作
- [ ] 确保 fluent-text-field 的 `value` 绑定和 `input` 事件在 Preact 中正常工作
- [ ] 确保 fluent-dialog 的 `showModal()`/`close()` 方法与 Preact 状态配合
- [ ] 验证 `noUnusedLocals` 和 `noUnusedParameters` 不会因为导入变化而报错

#### 8.3 视觉回归测试
- [ ] 确认 MainPage 布局正常（标题栏、搜索栏、标签栏、列表、底栏）
- [ ] 确认 SettingsPage 所有设置项正常
- [ ] 确认 Viewers（JSON/Image/Curl/WS/Calc/Decoder/Timestamp）正常工作
- [ ] 确认 Toast 窗口正常显示
- [ ] 确认 Modal（快捷键说明、错误提示、清空确认）正常
- [ ] 确认 ToggleSwitch（设置页切换开关）正常
- [ ] 确认队列弹出层正常
- [ ] 确认快捷键功能不受影响
- [ ] 确认窗口置顶交互不受影响

---

## 文件变更清单

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `package.json` | 修改 | 添加 `@fluentui/web-components` 依赖 |
| `src/main.tsx` | 修改 | 注册 Fluent 组件，包裹 DS Provider |
| `src/styles/global.css` | 修改 | 移除被 Fluent 替代的样式，保留自定义布局，添加亚克力 `.acrylic` 工具类 |
| `src-tauri/tauri.conf.json` | 修改 | 窗口添加 `"transparent": true` 以支持亚克力效果 |
| `src/components/modal.tsx` | 重写 | Modal → `fluent-dialog` |
| `src/components/toggle-switch.tsx` | 重写 | ToggleSwitch → `fluent-switch` |
| `src/components/shortcut-help.tsx` | 微调 | Modal 替换不影响 props |
| `src/components/queue-popup.tsx` | 不变 | 保持自定义 |
| `src/components/hotkey-editor.tsx` | 微调 | 按钮 → `fluent-button` |
| `src/components/action-module-list.tsx` | 微调 | ToggleSwitch/按钮 → Fluent 组件 |
| `src/components/fluent-icon.tsx` | 不变 | 保持现有 SVG 图标组件 |
| `src/routes/main/index.tsx` | 修改 | 替换按钮/输入/选择器等 |
| `src/routes/main/search-bar.tsx` | 修改 | input → `fluent-text-field`，select → `fluent-select` |
| `src/routes/main/entry-item.tsx` | 微调 | 按钮保持自定义 |
| `src/routes/main/entry-list.tsx` | 不变 | 保持自定义 |
| `src/routes/settings/index.tsx` | 修改 | 全面替换按钮/开关/卡片/滑块等 |
| `src/routes/viewer/json-view.tsx` | 微调 | 移除 CSS 类依赖，改用 inline style |
| `src/routes/viewer/image-view.tsx` | 修改 | 工具栏按钮 → `fluent-button` |
| `src/routes/viewer/curl-view.tsx` | 修改 | 全面替换输入/按钮/选择/卡片等 |
| `src/routes/viewer/ws-view.tsx` | 修改 | 替换按钮/输入/卡片等 |
| `src/routes/viewer/calc-view.tsx` | 修改 | 替换按键/输入/卡片等 |
| `src/routes/viewer/decoder-view.tsx` | 修改 | 替换 tabs/按钮/输入/卡片等 |
| `src/routes/viewer/timestamp-view.tsx` | 修改 | 替换按钮/输入/卡片等 |
| `src/routes/toast.tsx` | 微调 | 卡片 → `fluent-card` |
