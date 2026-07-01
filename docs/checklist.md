# jPaste v2 架构修复 Checklist

## 后端 (Rust) 修复项

### 严重问题
- [x] **BE-1**: 修复 `save_clipboard` 锁内文件 I/O（`service/history.rs`）✅
  - 将锁拆分为三个阶段：获取锁计算hash → 释放锁执行I/O → 获取锁写入数据
  - 同样修复 `delete_entry`、`cleanup`、`clear_all`
- [x] **BE-2**: 修复日志服务锁设计缺陷（`log_service.rs`）✅
  - `static mut COUNTER` 改为线程局部变量
  - Arc<Mutex<LogWriter>> 共享机制已正确
- [x] **BE-3**: 统一临时文件夹 + 应用退出清理 ✅
  - 新增 `util::jpaste_temp_dir()` 和 `util::cleanup_temp_dir()`
  - `clipboard.rs` 使用 `%TEMP%/jpaste2/clip_*.png`
  - `fileop.rs` 使用 `%TEMP%/jpaste2/edit_*.*`
  - `lib.rs` 监听 `tauri://close-requested` 清理整个目录
  - 移除 `mem::forget` 泄漏

### 中等问题
- [x] **BE-4**: 修复 LIKE 子句通配符转义（`repository.rs`）✅
  - 转义 `%`、`_`、`\` 字符，使用 `ESCAPE '\'`
- [x] **BE-5**: 拆分 lib.rs setup 闭包（`lib.rs`）✅
  - 提取 `setup_window_behavior`、`setup_cleanup_timer`、`setup_log_relay`
- [x] **BE-6**: 初始化 image_bytes_cache（`service/history.rs`）✅
  - `HistoryService::new()` 调用 `get_image_storage_bytes` 初始化
- [x] **BE-7**: 消除重复代码 ✅
  - `model::is_windows_path` 改为 `pub`，`action::folder_detect` 复用
- [x] **BE-8**: 减少全局可变状态（`lib.rs`）✅
  - `LAST_TOAST_HASH`/`TOAST_GEN` 封装为 `ToastState` 结构体

### 性能优化
- [x] **BP-1**: 修复剪贴板图片重复读取（`clipboard.rs`）✅
  - 直接尝试 `read_image_to_temp_file()`，避免先 `get_image()` 检查
- [x] **BP-2**: 缓存 VS Code 路径检测结果（`service/fileop.rs`）✅
  - 使用 `OnceCell<bool>` 缓存 `where code` 结果
- [x] **BP-3**: 优化 Base64 检测算法（`action.rs`）✅
  - `"+/= ".contains(c)` → `matches!(c, '+' | '/' | '=' | ' ')`
- [x] **BP-4**: 优化图片字节统计查询（`repository.rs`）✅
  - 先收集 paths 到 Vec，再统一做 file I/O

---

## 前端 (Preact) 修复项

### 严重问题
- [x] **FE-1**: 修复快捷键闭包过期 Bug ✅
  - `use-keyboard.ts` 使用 `useRef` 持有最新 shortcuts
  - 移除 `deps` 参数，listener 只注册一次

- [x] **FE-2**: 修复 math.ts new Function 安全风险 ✅
  - 使用手写 shunting-yard 算法替代 `new Function()`

### 中等问题
- [x] **FE-3**: 统一状态管理为 signals ✅
  - `use-filo-status.ts` 从 useState 改为 signals
  - `sortField`/`sortOrder` 从 useState 改为 signals
- [ ] **FE-4**: 拆分 God Components
  - `MainPage` 拆分为多个 hooks
  - `CurlViewPage` 提取子组件和工具函数
- [x] **FE-5**: 消除前端重复代码 ✅
  - `ACTION_ICONS` 提取到 `entry-item.tsx` 并导出，`action-module-list.tsx` 复用
  - `handleSelect`/`handleCopy` 合并（别名）
- [x] **FE-6**: 替换 alert() 为 Toast ✅
  - `math.ts`、`base64.ts`、`timestamp.ts`、`unicode.ts` 全部改用 `api.showToast()`

### 性能优化
- [x] **FP-1**: 缓存 detect() 调用结果（`routes/main/entry-item.tsx`）✅
  - `useMemo(() => detect(entry.content), [entry.content])`
- [x] **FP-2**: ActionModuleList 排序添加 useMemo ✅
  - `useMemo(() => [...modules].sort(...), [modules, actionConfig])`
- [x] **FP-3**: 修复 useCallback 依赖 churn ✅
  - 使用 `entriesRef` 存储最新 entries，回调不再依赖 `currentEntries`
- [x] **FP-4**: onMouseEnter 添加防抖（`components/queue-popup.tsx`）✅
  - 300ms debounce timer

---

## 架构深化（待后续迭代）

- [ ] **AD-1**: 图片处理逻辑提取为 ImageService
- [ ] **AD-2**: Command 层样板代码消除（宏或 trait）
- [ ] **AD-3**: Toast 窗口逻辑提取为独立模块
- [ ] **AD-4**: Settings 回调签名统一

---

*最后更新: 2026-07-03 - 全部修复完成（FE-4 和 AD 系列待后续迭代）*
