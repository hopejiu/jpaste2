import { Modal } from '../components/modal';

interface ShortcutHelpProps {
  open: boolean;
  onClose: () => void;
}

const SHORTCUT_GROUPS = [
  { title: '搜索', items: [
    { keys: 'Ctrl+L', desc: '聚焦搜索框' },
    { keys: 'Esc', desc: '清空搜索（有搜索词时）' },
  ] },
  { title: '编辑', items: [
    { keys: 'Ctrl+E', desc: '在编辑器中打开选中条目' },
    { keys: 'Ctrl+C', desc: '复制选中条目' },
    { keys: 'Delete', desc: '删除选中条目' },
    { keys: 'Space', desc: '切换收藏状态' },
  ] },
  { title: '导航', items: [
    { keys: '↑  ↓', desc: '在条目列表中上下移动焦点' },
    { keys: 'Ctrl+1~9', desc: '对第 N 条条目执行默认操作' },
    { keys: 'Enter', desc: '对焦点条目执行默认操作' },
    { keys: 'Home / End', desc: '滚动到列表顶部/底部' },
    { keys: 'PageUp / PageDown', desc: '按页滚动' },
  ] },
  { title: '标签', items: [
    { keys: '[  /  ]', desc: '切换 剪贴板 / 工具箱' },
    { keys: 'Tab / Shift+Tab', desc: '切换内容筛选标签' },
  ] },
  { title: '窗口', items: [
    { keys: 'Esc', desc: '隐藏窗口（无搜索词时）' },
    { keys: 'Alt+V', desc: '全局快捷键，显示/隐藏窗口' },
  ] },
  { title: '粘贴模式', hint: '底栏可切换两种粘贴方式，决定连续按 Ctrl+V 时的顺序', items: [
    { keys: '正常', desc: '始终粘贴最新复制的内容，与日常习惯完全一致' },
    { keys: '队列', desc: '像排队——先来先得。适合按复制顺序依次粘贴一段段内容' },
  ] },
];

export function ShortcutHelp({ open, onClose }: ShortcutHelpProps) {
  return (
    <Modal open={open} onClose={onClose} title="快捷键说明">
      <div class="shortcut-groups">
        {SHORTCUT_GROUPS.map((group) => (
          <div key={group.title} class="shortcut-group">
            <div class="shortcut-group-title">{group.title}</div>
            {group.hint && <div class="shortcut-group-hint">{group.hint}</div>}
            {group.items.map((item) => (
              <div key={item.keys} class="shortcut-row">
                <kbd class="shortcut-key">{item.keys}</kbd>
                <span class="shortcut-desc">{item.desc}</span>
              </div>
            ))}
          </div>
        ))}
      </div>
    </Modal>
  );
}
