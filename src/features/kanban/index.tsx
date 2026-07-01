//! 看板 — Kanban board viewer
//! Standalone Preact component backed by IndexedDB.
//! Imported as a toolbox viewer route (/viewer/kanban).

import { useEffect, useState, useRef, useCallback } from 'preact/hooks';
import { error as logError, setComponent } from '../../lib/logger';

setComponent('kanban');

/* ── IndexedDB helpers ─────────────────────────────────────────────────── */

const DB_NAME = 'KanbanDB', DB_VERSION = 3;

interface Column { id: string; title: string; order: number; }
interface Card { id: string; columnId: string; title: string; description: string; priority: string | null; dueDate: string | null; tags: TagRef[]; subtasks: Subtask[]; order: number; createdAt: number; }
interface TagRef { id: string; name: string; color: string; }
interface TagDef { id: string; name: string; color: string; }
interface Subtask { id: string; title: string; done: boolean; }

const DEFAULT_TAGS: TagDef[] = [
  { id: 'tag-design', name: '设计', color: '#8B5CF6' },
  { id: 'tag-dev', name: '开发', color: '#3B82F6' },
  { id: 'tag-bug', name: '缺陷', color: '#EF4444' },
  { id: 'tag-feature', name: '功能', color: '#10B981' },
  { id: 'tag-docs', name: '文档', color: '#F59E0B' },
];

let db: IDBDatabase | null = null;

function openDB(): Promise<IDBDatabase> {
  if (db) return Promise.resolve(db);
  return new Promise((resolve, reject) => {
    const r = indexedDB.open(DB_NAME, DB_VERSION);
    r.onerror = () => reject(r.error);
    r.onsuccess = () => { db = r.result; resolve(db!); };
    r.onupgradeneeded = (e) => {
      const d = (e.target as IDBOpenDBRequest).result;
      if (!d.objectStoreNames.contains('columns')) {
        const s = d.createObjectStore('columns', { keyPath: 'id' });
        s.createIndex('order', 'order', { unique: false });
      }
      if (!d.objectStoreNames.contains('cards')) {
        const s = d.createObjectStore('cards', { keyPath: 'id' });
        s.createIndex('columnId', 'columnId', { unique: false });
        s.createIndex('order', 'order', { unique: false });
      }
      if (!d.objectStoreNames.contains('tags')) d.createObjectStore('tags', { keyPath: 'id' });
      if (e.oldVersion === 1) {
        const tx = (e.target as IDBOpenDBRequest).transaction!;
        const store = tx.objectStore('cards');
        store.openCursor().onsuccess = (ev: Event) => {
          const cur = (ev.target as IDBRequest<IDBCursorWithValue>).result;
          if (cur) {
            const c = cur.value as any;
            if (!c.priority) c.priority = null;
            if (!c.dueDate) c.dueDate = null;
            if (!c.tags) c.tags = [];
            cur.update(c);
            cur.continue();
          }
        };
      }
      if (e.oldVersion < 3) {
        const tx = (e.target as IDBOpenDBRequest).transaction!;
        const store = tx.objectStore('cards');
        store.openCursor().onsuccess = (ev: Event) => {
          const cur = (ev.target as IDBRequest<IDBCursorWithValue>).result;
          if (cur) {
            const c = cur.value as any;
            if (!c.subtasks) { c.subtasks = []; cur.update(c); }
            cur.continue();
          }
        };
      }
    };
  });
}

function getAll<T>(name: string): Promise<T[]> {
  return new Promise((resolve, reject) => {
    const tx = db!.transaction(name, 'readonly');
    const q = tx.objectStore(name).getAll();
    q.onsuccess = () => resolve(q.result);
    q.onerror = () => reject(q.error);
  });
}

function add<T>(name: string, data: T): Promise<IDBValidKey> {
  return new Promise((resolve, reject) => {
    const q = db!.transaction(name, 'readwrite').objectStore(name).add(data);
    q.onsuccess = () => resolve(q.result);
    q.onerror = () => reject(q.error);
  });
}

function put<T>(name: string, data: T): Promise<IDBValidKey> {
  return new Promise((resolve, reject) => {
    const q = db!.transaction(name, 'readwrite').objectStore(name).put(data);
    q.onsuccess = () => resolve(q.result);
    q.onerror = () => reject(q.error);
  });
}

function del(name: string, id: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const q = db!.transaction(name, 'readwrite').objectStore(name).delete(id);
    q.onsuccess = () => resolve();
    q.onerror = () => reject(q.error);
  });
}

function batchPut<T extends { id: string }>(name: string, items: T[]): Promise<void> {
  if (!items.length) return Promise.resolve();
  return new Promise((resolve, reject) => {
    const store = db!.transaction(name, 'readwrite').objectStore(name);
    let done = 0;
    for (const i of items) {
      const q = store.put(i);
      q.onsuccess = () => { if (++done === items.length) resolve(); };
      q.onerror = () => reject(q.error);
    }
  });
}

function batchDel(name: string, ids: string[]): Promise<void> {
  if (!ids.length) return Promise.resolve();
  return new Promise((resolve, reject) => {
    const store = db!.transaction(name, 'readwrite').objectStore(name);
    let done = 0;
    for (const id of ids) {
      const q = store.delete(id);
      q.onsuccess = () => { if (++done === ids.length) resolve(); };
      q.onerror = () => reject(q.error);
    }
  });
}

/* ── Utility fns ───────────────────────────────────────────────────────── */

function formatDate(s: string | null): string {
  if (!s) return '';
  const d = new Date(s);
  return `${d.getMonth() + 1}月${d.getDate()}日`;
}

function getDueDateStatus(s: string | null): string {
  if (!s) return 'n';
  const now = new Date(); now.setHours(0, 0, 0, 0);
  const d = new Date(s); d.setHours(0, 0, 0, 0);
  const diff = d.getTime() - now.getTime();
  if (diff < 0) return 'overdue';
  if (diff <= 259200000) return 'dueSoon';
  return 'normal';
}

function nowId(): string { return crypto.randomUUID(); }

/* ── Column CRUD ───────────────────────────────────────────────────────── */

async function createColumn(title: string): Promise<Column> {
  const cols = await getAll<Column>('columns');
  const c: Column = { id: nowId(), title, order: cols.length };
  await add('columns', c);
  return c;
}

async function updateColumn(id: string, title: string): Promise<void> {
  const cols = await getAll<Column>('columns');
  const c = cols.find(x => x.id === id);
  if (c) { c.title = title; await put('columns', c); }
}

async function deleteColumn(id: string): Promise<void> {
  const all = await getAll<Card>('cards');
  await batchDel('cards', all.filter(c => c.columnId === id).map(c => c.id));
  await del('columns', id);
  const cols = await getAll<Column>('columns');
  cols.sort((a, b) => a.order - b.order);
  cols.forEach((c, i) => c.order = i);
  await batchPut('columns', cols);
}

/* ── Card CRUD ─────────────────────────────────────────────────────────── */

async function createCard(colId: string, title: string, desc = '', priority: string | null = null, dueDate: string | null = null, tags: TagRef[] = []): Promise<Card> {
  const all = await getAll<Card>('cards');
  const cc = all.filter(c => c.columnId === colId);
  const c: Card = { id: nowId(), columnId: colId, title, description: desc, priority, dueDate, tags, subtasks: [], order: cc.length, createdAt: Date.now() };
  await add('cards', c);
  return c;
}

async function updateCardData(id: string, data: Partial<Card>): Promise<void> {
  const all = await getAll<Card>('cards');
  const c = all.find(x => x.id === id);
  if (c) { Object.assign(c, data); await put('cards', c); }
}

async function deleteCard(id: string): Promise<void> {
  const all = await getAll<Card>('cards');
  const c = all.find(x => x.id === id);
  if (!c) return;
  await del('cards', id);
  const cc = all.filter(x => x.columnId === c.columnId && x.id !== id);
  cc.sort((a, b) => a.order - b.order);
  cc.forEach((x, i) => x.order = i);
  await batchPut('cards', cc);
}

async function moveCard(cardId: string, newColId: string, newOrder: number): Promise<void> {
  const all = await getAll<Card>('cards');
  const card = all.find(c => c.id === cardId);
  if (!card) return;
  const oldCol = card.columnId;
  card.columnId = newColId;
  card.order = newOrder;
  await put('cards', card);
  const updates: Card[] = [];
  const tgt = all.filter(c => c.columnId === newColId && c.id !== cardId);
  tgt.sort((a, b) => a.order - b.order);
  tgt.forEach((c, i) => { c.order = i >= newOrder ? i + 1 : i; updates.push(c); });
  if (oldCol !== newColId) {
    const src = all.filter(c => c.columnId === oldCol);
    src.sort((a, b) => a.order - b.order);
    src.forEach((c, i) => { c.order = i; updates.push(c); });
  }
  await batchPut('cards', updates);
}

/* ── Tags ──────────────────────────────────────────────────────────────── */

async function initTags(): Promise<void> {
  const t = await getAll<TagDef>('tags');
  if (!t.length) for (const tag of DEFAULT_TAGS) await add('tags', tag);
}

/* ── Component ─────────────────────────────────────────────────────────── */

interface FilterState { priority: string; dueDate: string; }

export function KanbanPage() {
  const [columns, setColumns] = useState<Column[]>([]);
  const [allCards, setAllCards] = useState<Card[]>([]);
  const [tags, setTags] = useState<TagDef[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [filterState, setFilterState] = useState<FilterState>({ priority: 'all', dueDate: 'all' });
  const [loading, setLoading] = useState(true);

  // Detail panel
  const [detailCardId, setDetailCardId] = useState<string | null>(null);
  const [detailTitle, setDetailTitle] = useState('');
  const [detailDesc, setDetailDesc] = useState('');
  const [detailPriority, setDetailPriority] = useState<string | null>(null);
  const [detailDueDate, setDetailDueDate] = useState('');
  const [detailTags, setDetailTags] = useState<TagRef[]>([]);
  const [detailSubtasks, setDetailSubtasks] = useState<Subtask[]>([]);
  const [subtaskInput, setSubtaskInput] = useState('');

  // Modals
  const [columnModalOpen, setColumnModalOpen] = useState(false);
  const [editingColumnId, setEditingColumnId] = useState<string | null>(null);
  const [columnFormTitle, setColumnFormTitle] = useState('');
  const [deleteModalOpen, setDeleteModalOpen] = useState(false);
  const [deleteMessage, setDeleteMessage] = useState('');
  const [deleteCallback, setDeleteCallback] = useState<(() => Promise<void>) | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);

  // Quick add inline editing
  const [quickAddColId, setQuickAddColId] = useState<string | null>(null);
  const quickAddRef = useRef<HTMLTextAreaElement>(null);
  const [filterDropdown, setFilterDropdown] = useState<'priority' | 'dueDate' | null>(null);
  const [tagSelectorOpen, setTagSelectorOpen] = useState(false);
  const [toastMsg, setToastMsg] = useState<{ msg: string; type: string; key: number } | null>(null);

  // Drag state (managed by dragState + refs in the manual drag handler)

  // New card animation tracking
  const newCardIds = useRef<Set<string>>(new Set());
  const [, forceRender] = useState(0);

  // Debounce timer ref for auto-save
  const autoSaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const colAutoSaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const panelOpenCardId = useRef<string | null>(null);

  const showToast = useCallback((msg: string, type = 'default') => {
    setToastMsg({ msg, type, key: Date.now() });
    setTimeout(() => setToastMsg(null), 3000);
  }, []);

  // ── Load data ──
  const loadData = useCallback(async () => {
    try {
      await openDB();
      await initTags();
      const [cols, cards, t] = await Promise.all([
        getAll<Column>('columns'),
        getAll<Card>('cards'),
        getAll<TagDef>('tags'),
      ]);
      if (cols.length === 0) {
        // First time: init demo data
        await createColumn('待办');
        await createColumn('进行中');
        await createColumn('已完成');
        const allCols = await getAll<Column>('columns');
        await createCard(allCols[0].id, '欢迎使用看板工具', '这是一个示例卡片，你可以拖拽它到其他列，或者编辑它的内容。', 'medium', '2026-05-15', [{ id: 'tag-feature', name: '功能', color: '#10B981' }]);
        await createCard(allCols[0].id, '创建新卡片', '点击列底部的添加按钮快速创建卡片');
        await createCard(allCols[1].id, '拖拽卡片', '拖拽卡片到其他列来改变它们的状态', 'low', '', [{ id: 'tag-dev', name: '开发', color: '#3B82F6' }]);
        await createCard(allCols[2].id, '编辑或删除', '使用编辑和删除按钮管理卡片', '', '', [{ id: 'tag-docs', name: '文档', color: '#F59E0B' }]);
        // Add demo subtasks to first card
        const demoCards = await getAll<Card>('cards');
        const welcomeCard = demoCards.find(c => c.title === '欢迎使用看板工具');
        if (welcomeCard) {
          welcomeCard.subtasks = [
            { id: nowId(), title: '尝试拖拽卡片', done: true },
            { id: nowId(), title: '点击编辑详情', done: false },
            { id: nowId(), title: '添加子任务', done: false },
          ];
          await put('cards', welcomeCard);
        }
        const [cols2, cards2, t2] = await Promise.all([
          getAll<Column>('columns'),
          getAll<Card>('cards'),
          getAll<TagDef>('tags'),
        ]);
        setColumns(cols2.sort((a, b) => a.order - b.order));
        setAllCards(cards2);
        setTags(t2);
      } else {
        setColumns(cols.sort((a, b) => a.order - b.order));
        setAllCards(cards);
        setTags(t);
      }
    } catch (e) {
      logError('kanban:load', e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadData(); }, []);

  // ── Filter helpers ──
  const filteredCards = allCards.filter(c => {
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      if (!c.title.toLowerCase().includes(q) && !(c.description && c.description.toLowerCase().includes(q))) return false;
    }
    if (filterState.priority !== 'all') {
      if (filterState.priority === 'none') { if (c.priority) return false; }
      else if (c.priority !== filterState.priority) return false;
    }
    if (filterState.dueDate !== 'all') {
      if (filterState.dueDate === 'overdue' && (!c.dueDate || getDueDateStatus(c.dueDate) !== 'overdue')) return false;
      if (filterState.dueDate === 'dueSoon' && (!c.dueDate || getDueDateStatus(c.dueDate) !== 'dueSoon')) return false;
      if (filterState.dueDate === 'noDate' && c.dueDate) return false;
    }
    return true;
  });

  const filterBadge = (type: 'priority' | 'dueDate') => {
    const val = filterState[type];
    if (val === 'all') return null;
    const labels: Record<string, string> = { high: '高', medium: '中', low: '低', none: '无', overdue: '过期', dueSoon: '即将', noDate: '无期' };
    return labels[val] || null;
  };

  // ── Detail Panel ──
  const openDetailPanel = useCallback(async (cardId: string | null, colId: string | null) => {
    if (cardId) {
      const cards = await getAll<Card>('cards');
      const c = cards.find(x => x.id === cardId);
      if (c) {
        setDetailCardId(c.id);
        setDetailTitle(c.title);
        setDetailDesc(c.description || '');
        setDetailPriority(c.priority || null);
        setDetailDueDate(c.dueDate || '');
        setDetailTags(c.tags ? [...c.tags] : []);
        setDetailSubtasks(c.subtasks ? [...c.subtasks] : []);
        setSubtaskInput('');
        panelOpenCardId.current = c.id;
      }
    } else {
      // Create card immediately for real-time editing
      if (colId) {
        const newCard = await createCard(colId, '');
        setDetailCardId(newCard.id);
        setDetailTitle('');
        setDetailDesc('');
        setDetailPriority(null);
        setDetailDueDate('');
        setDetailTags([]);
        setDetailSubtasks([]);
        setSubtaskInput('');
        panelOpenCardId.current = newCard.id;
        newCardIds.current.add(newCard.id);
        const all = await getAll<Card>('cards');
        setAllCards(all);
      }
    }
  }, []);

  const closeDetailPanel = useCallback(async () => {
    if (autoSaveTimer.current) clearTimeout(autoSaveTimer.current);
    // If new card with empty title, delete it (user cancelled)
    if (detailCardId && !detailTitle.trim()) {
      await deleteCard(detailCardId);
      const all = await getAll<Card>('cards');
      setAllCards(all);
      newCardIds.current.delete(detailCardId);
    }
    setDetailCardId(null);
    panelOpenCardId.current = null;
    setTagSelectorOpen(false);
    setSubtaskInput('');
  }, [detailCardId, detailTitle]);

  const debouncedAutoSave = useCallback(() => {
    if (autoSaveTimer.current) clearTimeout(autoSaveTimer.current);
    autoSaveTimer.current = setTimeout(async () => {
      if (!panelOpenCardId.current || !detailTitle.trim()) return;
      await updateCardData(panelOpenCardId.current, {
        title: detailTitle.trim(),
        description: detailDesc.trim(),
        priority: detailPriority,
        dueDate: detailDueDate || null,
        tags: detailTags,
        subtasks: detailSubtasks,
      });
      const all = await getAll<Card>('cards');
      setAllCards(all);
    }, 800);
  }, [detailTitle, detailDesc, detailPriority, detailDueDate, detailTags, detailSubtasks]);

  useEffect(() => {
    if (panelOpenCardId.current) debouncedAutoSave();
  }, [detailTitle, detailDesc, detailPriority, detailDueDate, detailTags, detailSubtasks]);

  // ── Subtask CRUD ──
  const addSubtask = useCallback(() => {
    const t = subtaskInput.trim();
    if (!t) return;
    setDetailSubtasks(prev => [...prev, { id: nowId(), title: t, done: false }]);
    setSubtaskInput('');
  }, [subtaskInput]);

  const toggleSubtask = useCallback((subtaskId: string) => {
    setDetailSubtasks(prev => prev.map(s => s.id === subtaskId ? { ...s, done: !s.done } : s));
  }, []);

  const deleteSubtask = useCallback((subtaskId: string) => {
    setDetailSubtasks(prev => prev.filter(s => s.id !== subtaskId));
  }, []);

  const deleteCardFromPanel = useCallback(async () => {
    if (!detailCardId) return;
    await deleteCard(detailCardId);
    closeDetailPanel();
    const all = await getAll<Card>('cards');
    setAllCards(all);
    showToast('卡片已删除', 'default');
  }, [detailCardId, closeDetailPanel, showToast]);



  // ── Manual drag-and-drop with mousedown/mousemove/mouseup ──
  // ponytail: Tauri v2 WebView2 intercepts HTML5 DragEvent API (no dragover/drop
  // reaches the DOM).  Manual mouse tracking bypasses this entirely.
  const [dragState, setDragState] = useState<{ cardId: string; cardEl: HTMLElement } | null>(null);
  const dragGhostRef = useRef<HTMLElement | null>(null);
  const dragOverColumnRef = useRef<string | null>(null);

  useEffect(() => {
    if (!dragState) return;
    const cardEl = dragState.cardEl;
    const cardId = dragState.cardId;
    let moved = false;
    let placeholder: HTMLElement | null = null;

    const onMove = (e: MouseEvent) => {
      if (!moved) {
        moved = true;
        cardEl.classList.add('dragging');
        // Create ghost (semi-transparent clone)
        const ghost = cardEl.cloneNode(true) as HTMLElement;
        ghost.className = 'kanban-card';
        ghost.style.position = 'fixed';
        ghost.style.pointerEvents = 'none';
        ghost.style.opacity = '0.7';
        ghost.style.zIndex = '9999';
        ghost.style.width = cardEl.offsetWidth + 'px';
        ghost.style.transform = 'rotate(2deg) scale(1.02)';
        ghost.style.boxShadow = '0 8px 32px rgba(0,0,0,0.12)';
        ghost.style.cursor = 'grabbing';
        document.body.appendChild(ghost);
        dragGhostRef.current = ghost;
      }

      const ghostEl = dragGhostRef.current;
      if (ghostEl) {
        ghostEl.style.left = (e.clientX - 10) + 'px';
        ghostEl.style.top = (e.clientY - 10) + 'px';
      }

      // Find which column the cursor is over
      const el = document.elementFromPoint(e.clientX, e.clientY);
      const col = el?.closest<HTMLElement>('.kanban-column');
      const newColId = col?.dataset.columnId ?? null;

      // Clean old placeholder if column changed
      if (newColId !== dragOverColumnRef.current) {
        document.querySelectorAll('.drag-placeholder').forEach(p => p.remove());
        document.querySelectorAll('.kanban-cards.drag-over').forEach(el => el.classList.remove('drag-over'));
        dragOverColumnRef.current = newColId;
      }

      if (col && newColId) {
        const ct = col.querySelector<HTMLElement>('.kanban-cards');
        if (ct) {
          ct.classList.add('drag-over');
          // Find insertion index
          const cards = Array.from(ct.querySelectorAll<HTMLElement>('.kanban-card:not(.dragging)'));
          let insertBefore: HTMLElement | null = null;
          for (const card of cards) {
            const rect = card.getBoundingClientRect();
            if (e.clientY < rect.top + rect.height / 2) { insertBefore = card; break; }
          }
          // Create or move placeholder
          if (!placeholder || placeholder.parentNode !== ct) {
            placeholder?.remove();
            placeholder = document.createElement('div');
            placeholder.className = 'drag-placeholder';
            ct.appendChild(placeholder);
          }
          if (insertBefore) ct.insertBefore(placeholder, insertBefore);
          else ct.appendChild(placeholder);
        }
      }
    };

    const onUp = async (_e: MouseEvent) => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      cardEl.classList.remove('dragging');
      const ghost = dragGhostRef.current;
      if (ghost) { ghost.remove(); dragGhostRef.current = null; }
      const targetColId = dragOverColumnRef.current;
      dragOverColumnRef.current = null;

      if (targetColId) {
        const col = document.querySelector<HTMLElement>(`.kanban-column[data-column-id="${targetColId}"]`);
        const ct = col?.querySelector<HTMLElement>('.kanban-cards');
        if (ct) {
          ct.classList.remove('drag-over');
          const ph = ct.querySelector('.drag-placeholder');
          let newOrder = Array.from(ct.querySelectorAll('.kanban-card:not(.dragging)')).length;
          if (ph) {
            let idx = 0;
            for (const ch of Array.from(ct.children)) {
              if (ch === ph) break;
              if (ch.classList.contains('kanban-card') && !ch.classList.contains('dragging')) idx++;
            }
            newOrder = idx;
            ph.remove();
          }
          await moveCard(cardId, targetColId, newOrder);
          const all = await getAll<Card>('cards');
          setAllCards(all);
          newCardIds.current.delete(cardId);
        }
      } else {
        document.querySelectorAll('.drag-placeholder').forEach(p => p.remove());
        document.querySelectorAll('.kanban-cards.drag-over').forEach(el => el.classList.remove('drag-over'));
      }
      setDragState(null);
    };

    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
    return () => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
    };
  }, [dragState]);

  // ── Quick Add ──
  const submitQuickAdd = useCallback(async (colId: string) => {
    const inp = quickAddRef.current;
    if (!inp) return;
    const t = inp.value.trim();
    if (!t) return;
    const newCard = await createCard(colId, t);
    newCardIds.current.add(newCard.id);
    inp.value = '';
    const all = await getAll<Card>('cards');
    setAllCards(all);
    forceRender(n => n + 1);
    inp.focus();
  }, []);

  // ── Column modal ──
  const openColumnModal = useCallback(async (colId: string | null = null) => {
    if (colId) {
      setEditingColumnId(colId);
      setColumnFormTitle(columns.find(c => c.id === colId)?.title || '');
    } else {
      // Create column immediately for real-time editing
      const newCol = await createColumn('');
      setEditingColumnId(newCol.id);
      setColumnFormTitle('');
    }
    setColumnModalOpen(true);
  }, [columns]);

  const handleColumnTitleInput = useCallback((title: string) => {
    setColumnFormTitle(title);
    if (colAutoSaveTimer.current) clearTimeout(colAutoSaveTimer.current);
    if (!editingColumnId) return;
    colAutoSaveTimer.current = setTimeout(async () => {
      await updateColumn(editingColumnId, title);
      const cols = await getAll<Column>('columns');
      setColumns(cols.sort((a, b) => a.order - b.order));
    }, 500);
  }, [editingColumnId]);

  const closeColumnModal = useCallback(async () => {
    if (colAutoSaveTimer.current) clearTimeout(colAutoSaveTimer.current);
    // If new column with empty title, delete it
    if (editingColumnId && !columnFormTitle.trim()) {
      await deleteColumn(editingColumnId);
      const cols = await getAll<Column>('columns');
      setColumns(cols.sort((a, b) => a.order - b.order));
      const all = await getAll<Card>('cards');
      setAllCards(all);
    }
    setColumnModalOpen(false);
  }, [editingColumnId, columnFormTitle]);

  const confirmDeleteColumn = useCallback(async (colId: string) => {
    setDeleteMessage('确定要删除此列及其所有卡片吗？此操作不可撤销。');
    setDeleteCallback(() => async () => {
      await deleteColumn(colId);
      const cols = await getAll<Column>('columns');
      setColumns(cols.sort((a, b) => a.order - b.order));
      const all = await getAll<Card>('cards');
      setAllCards(all);
      showToast('列已删除', 'success');
    });
    setDeleteModalOpen(true);
  }, [showToast]);

  const confirmDeleteCard = useCallback(async (cardId: string) => {
    setDeleteMessage('确定要删除这张卡片吗？此操作不可撤销。');
    setDeleteCallback(() => async () => {
      await deleteCard(cardId);
      const all = await getAll<Card>('cards');
      setAllCards(all);
      showToast('卡片已删除', 'default');
    });
    setDeleteModalOpen(true);
  }, [allCards, showToast]);

  // ── Settings ──
  const [cachedColumns, setCachedColumns] = useState<Column[]>([]);
  const [settingsTags, setSettingsTags] = useState<TagDef[]>([]);
  const [newTagName, setNewTagName] = useState('');
  const [newTagColor, setNewTagColor] = useState('#2563EB');

  const openSettings = useCallback(async () => {
    const cols = await getAll<Column>('columns');
    setCachedColumns(cols.sort((a, b) => a.order - b.order));
    setSettingsTags(await getAll<TagDef>('tags'));
    setSettingsOpen(true);
  }, []);

  const handleMoveCol = useCallback(async (id: string, dir: 'up' | 'down') => {
    const cols = [...cachedColumns];
    const i = cols.findIndex(c => c.id === id);
    if (i < 0) return;
    const swap = dir === 'up' ? i - 1 : i + 1;
    if (swap < 0 || swap >= cols.length) return;
    [cols[i], cols[swap]] = [cols[swap], cols[i]];
    cols.forEach((c, idx) => c.order = idx);
    await batchPut('columns', cols);
    setCachedColumns([...cols]);
    const allCols = await getAll<Column>('columns');
    setColumns(allCols.sort((a, b) => a.order - b.order));
  }, [cachedColumns]);

  const addNewTag = useCallback(async () => {
    if (!newTagName.trim()) { showToast('请输入标签名称', 'warning'); return; }
    await add('tags', { id: 'tag-' + nowId(), name: newTagName.trim(), color: newTagColor });
    setNewTagName('');
    setSettingsTags(await getAll<TagDef>('tags'));
    setTags(await getAll<TagDef>('tags'));
    showToast('标签已添加', 'success');
  }, [newTagName, newTagColor, showToast]);

  const deleteTag = useCallback(async (tagId: string) => {
    await del('tags', tagId);
    const cards = await getAll<Card>('cards');
    const updates = cards.filter(c => c.tags && c.tags.some(t => t.id === tagId));
    updates.forEach(c => { c.tags = c.tags.filter(t => t.id !== tagId); });
    if (updates.length) await batchPut('cards', updates);
    setSettingsTags(await getAll<TagDef>('tags'));
    setTags(await getAll<TagDef>('tags'));
    const all = await getAll<Card>('cards');
    setAllCards(all);
    showToast('标签已删除', 'success');
  }, [showToast]);

  // ── Import / Export / Reset ──
  const exportData = useCallback(async () => {
    try {
      const [cols, cards, t] = await Promise.all([getAll<Column>('columns'), getAll<Card>('cards'), getAll<TagDef>('tags')]);
      const data = { version: '2.0', exportDate: new Date().toISOString(), columns: cols, cards, tags: t };
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `kanban-${new Date().toISOString().slice(0, 10)}.json`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
      showToast('数据已导出', 'success');
    } catch (e) { showToast('导出失败', 'error'); }
  }, [showToast]);

  const importData = useCallback(async (e: Event) => {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    try {
      const text = await file.text();
      const data = JSON.parse(text);
      if (!data.columns || !data.cards) throw new Error('无效的数据格式');
      setDeleteMessage('导入数据将清空当前所有数据，确定继续吗？');
      setDeleteCallback(() => async () => {
        const [ec, ecl] = await Promise.all([getAll<Card>('cards'), getAll<Column>('columns')]);
        await Promise.all([batchDel('cards', ec.map(c => c.id)), batchDel('columns', ecl.map(c => c.id))]);
        for (const c of data.columns) await add('columns', c);
        for (const c of data.cards) await add('cards', c);
        if (data.tags) for (const t of data.tags) await add('tags', t);
        const [cols2, cards2, tags2] = await Promise.all([getAll<Column>('columns'), getAll<Card>('cards'), getAll<TagDef>('tags')]);
        setColumns(cols2.sort((a, b) => a.order - b.order));
        setAllCards(cards2);
        setTags(tags2);
        showToast('数据导入成功', 'success');
      });
      setDeleteModalOpen(true);
    } catch (e: any) { showToast('导入失败：' + (e.message || e), 'error'); }
    input.value = '';
  }, [showToast]);

  const resetBoard = useCallback(async () => {
    setDeleteMessage('确定要重置看板吗？所有数据将被清空。');
    setDeleteCallback(() => async () => {
      const [cards, cols] = await Promise.all([getAll<Card>('cards'), getAll<Column>('columns')]);
      await Promise.all([batchDel('cards', cards.map(c => c.id)), batchDel('columns', cols.map(c => c.id))]);
      setColumns([]);
      setAllCards([]);
      showToast('看板已重置', 'success');
    });
    setDeleteModalOpen(true);
  }, [showToast]);

  // ── Close dropdowns on external click ──
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      const t = e.target as HTMLElement;
      if (!t.closest('.filter-group')) setFilterDropdown(null);
      if (!t.closest('.tag-selector')) setTagSelectorOpen(false);
    };
    document.addEventListener('click', handler);
    return () => document.removeEventListener('click', handler);
  }, []);

  // ── Keyboard shortcuts ──
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (deleteModalOpen || columnModalOpen || settingsOpen) return; // handled by overlay
        if (detailCardId !== undefined && detailCardId !== null) { closeDetailPanel(); return; }
        if (searchQuery) { setSearchQuery(''); return; }
        return;
      }
      const active = document.activeElement;
      const inInput = active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA' || (active as HTMLElement).isContentEditable);
      if (inInput) return;
      if (e.key === 'n' || e.key === 'N') { e.preventDefault(); const firstCol = document.querySelector('.kanban-quick-add-trigger') as HTMLElement; firstCol?.click(); }
      if (e.key === '/') { e.preventDefault(); document.querySelector<HTMLInputElement>('.kanban-search')?.focus(); }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [searchQuery, deleteModalOpen, columnModalOpen, settingsOpen, detailCardId, closeDetailPanel]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') { e.stopPropagation(); window.history.back(); }
    };
    document.addEventListener('keydown', handler, true);
    return () => document.removeEventListener('keydown', handler, true);
  }, []);

  if (loading) {
    return <div class="kanban"><div class="kanban-loading">加载中…</div></div>;
  }

  const fileInputRef = useRef<HTMLInputElement>(null);

  return (
    <div class="kanban">
      {/* Top bar */}
      <div class="kanban-topbar">
        <input type="text" class="kanban-board-title" value="我的看板" spellcheck={false} readOnly />

        <div class="kanban-search-box">
          <span class="kanban-search-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
          </span>
          <input
            class="kanban-search"
            type="text"
            placeholder="搜索卡片..."
            value={searchQuery}
            onInput={(e) => setSearchQuery((e.target as HTMLInputElement).value)}
            aria-label="搜索卡片"
          />
          {searchQuery && (
            <button class="kanban-search-clear" onClick={() => setSearchQuery('')} aria-label="清除搜索">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
            </button>
          )}
          <span class="kanban-search-hint">/</span>
        </div>

        <div class="filter-group">
          <div style={{ position: 'relative' }}>
            <button class={`kanban-filter-btn ${filterState.priority !== 'all' ? 'active' : ''}`} onClick={() => setFilterDropdown(f => f === 'priority' ? null : 'priority')}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 15s1-1 4-1 5 2 8 2 4-1 4-1V3s-1 1-4 1-5-2-8-2-4 1-4 1z"/><line x1="4" y1="22" x2="4" y2="15"/></svg>
              优先级
              {filterBadge('priority') && <span class="kanban-filter-badge visible">{filterBadge('priority')}</span>}
            </button>
            {filterDropdown === 'priority' && (
              <div class="kanban-filter-dropdown show">
                <div class="kanban-filter-dropdown-title">优先级筛选</div>
                {[['all', '全部', '#9CA3AF'], ['high', '高优先级', '#EF4444'], ['medium', '中优先级', '#F59E0B'], ['low', '低优先级', '#3B82F6'], ['none', '无优先级', '#E5E7EB']].map(([val, label, color]) => (
                  <div key={val} class={`kanban-filter-option ${filterState.priority === val ? 'selected' : ''}`} onClick={() => { setFilterState(s => ({ ...s, priority: val })); setFilterDropdown(null); }}>
                    <span class="dot" style={{ background: color }}></span> {label}
                  </div>
                ))}
              </div>
            )}
          </div>
          <div style={{ position: 'relative' }}>
            <button class={`kanban-filter-btn ${filterState.dueDate !== 'all' ? 'active' : ''}`} onClick={() => setFilterDropdown(f => f === 'dueDate' ? null : 'dueDate')}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="4" width="18" height="18" rx="2" ry="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/></svg>
              截止日期
              {filterBadge('dueDate') && <span class="kanban-filter-badge visible">{filterBadge('dueDate')}</span>}
            </button>
            {filterDropdown === 'dueDate' && (
              <div class="kanban-filter-dropdown show">
                <div class="kanban-filter-dropdown-title">截止日期筛选</div>
                {[['all', '全部'], ['overdue', '已过期'], ['dueSoon', '即将到期'], ['noDate', '无日期']].map(([val, label]) => (
                  <div key={val} class={`kanban-filter-option ${filterState.dueDate === val ? 'selected' : ''}`} onClick={() => { setFilterState(s => ({ ...s, dueDate: val })); setFilterDropdown(null); }}>
                    {label}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        <div class="kanban-actions">
          <button class="kanban-action-btn" onClick={openSettings} title="设置" aria-label="设置">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
          </button>
          <button class="kanban-action-btn" onClick={exportData} title="导出" aria-label="导出">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
          </button>
          <button class="kanban-action-btn" onClick={() => fileInputRef.current?.click()} title="导入" aria-label="导入">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
          </button>
          <input ref={fileInputRef} type="file" accept=".json" style={{ display: 'none' }} onChange={importData} />
          <button class="kanban-action-btn kanban-action-btn-danger" onClick={resetBoard} title="重置" aria-label="重置">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10"/></svg>
          </button>
        </div>
      </div>

      {/* Board */}
      <div class="kanban-board">
        {columns.map(col => {
          const cards = filteredCards.filter(c => c.columnId === col.id).sort((a, b) => a.order - b.order);
          return (
            <div key={col.id} class="kanban-column" data-column-id={col.id}>
              <div class="kanban-column-header">
                <div class="kanban-column-header-left">
                  <h3>{col.title}</h3>
                  <span class="kanban-card-count">{cards.length}</span>
                </div>
                <div class="kanban-column-actions">
                  <button class="kanban-add-btn" title="添加卡片" onClick={() => setQuickAddColId(col.id)}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
                  </button>
                  <button class="kanban-edit-btn" title="编辑列" onClick={() => openColumnModal(col.id)}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
                  </button>
                  <button class="kanban-delete-btn" title="删除列" onClick={() => confirmDeleteColumn(col.id)}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
                  </button>
                </div>
              </div>
              <div
                class="kanban-cards"
              >
                {cards.length === 0 ? (
                  <div class="kanban-empty">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"/><line x1="12" y1="8" x2="12" y2="16"/><line x1="8" y1="12" x2="16" y2="12"/></svg>
                    <div>暂无卡片</div>
                  </div>
                ) : cards.map(card => (
                  <div
                    key={card.id}
                    class={`kanban-card ${newCardIds.current.has(card.id) ? 'kanban-card-entering' : ''}`}
                    data-card-id={card.id}
                    onAnimationEnd={() => { newCardIds.current.delete(card.id); forceRender(n => n + 1); }}
                    onClick={() => openDetailPanel(card.id, null)}
                    onMouseDown={() => { setDragState({ cardId: card.id, cardEl: (document.querySelector(`[data-card-id="${card.id}"]`) as HTMLElement) }); }}
                  >
                    {card.priority && <div class={`kanban-card-priority-bar ${card.priority}`}></div>}
                    <div class="kanban-card-actions" onClick={e => e.stopPropagation()}>
                      <button class="kanban-delete-btn" title="删除" onClick={() => confirmDeleteCard(card.id)}>
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
                      </button>
                    </div>
                    <div class="kanban-card-title">{card.title}</div>
                    {card.description && <div class="kanban-card-desc">{card.description}</div>}
                    <div class="kanban-card-meta">
                      {card.tags && card.tags.length > 0 && (
                        <div class="kanban-card-tags">
                          {card.tags.map(t => (
                            <span key={t.id} class="kanban-tag" style={{ background: t.color + '18', color: t.color }}>{t.name}</span>
                          ))}
                        </div>
                      )}
                      {card.dueDate && (() => {
                        const st = getDueDateStatus(card.dueDate);
                        const cls = st === 'overdue' ? 'overdue' : st === 'dueSoon' ? 'due-soon' : '';
                        return (
                          <span class={`kanban-card-due-date ${cls}`}>
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
                            {formatDate(card.dueDate)}
                          </span>
                        );
                      })()}
                      {card.subtasks && card.subtasks.length > 0 && (() => {
                        const done = card.subtasks.filter(s => s.done).length;
                        const total = card.subtasks.length;
                        return (
                          <span class={`kanban-card-subtask-progress ${done === total ? 'complete' : ''}`}>
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="9 11 12 14 22 4"/><path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11"/></svg>
                            {done}/{total}
                          </span>
                        );
                      })()}
                    </div>
                  </div>
                ))}
              </div>
              {quickAddColId === col.id ? (
                <div class="kanban-quick-add">
                  <textarea ref={quickAddRef} class="kanban-quick-add-input" placeholder="输入卡片标题，回车添加..." rows={1}
                    onKeyDown={(e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); submitQuickAdd(col.id); } }}
                  />
                  <div class="kanban-quick-add-actions">
                    <button class="kanban-btn-primary" onClick={() => submitQuickAdd(col.id)}>添加</button>
                    <button class="kanban-btn-ghost" onClick={() => setQuickAddColId(null)}>取消</button>
                    <span class="kanban-quick-add-hint">Enter 添加 · Shift+Enter 换行</span>
                  </div>
                </div>
              ) : (
                <button class="kanban-quick-add-trigger" onClick={() => setQuickAddColId(col.id)}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
                  添加卡片
                </button>
              )}
            </div>
          );
        })}
        <button class="kanban-add-column-btn" onClick={() => openColumnModal(null)}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
          添加新列
        </button>
      </div>

      {/* Detail Side Panel */}
      {detailCardId !== null && (
        <>
          <div class="kanban-overlay" onClick={closeDetailPanel}></div>
          <div class="kanban-detail-panel active">
            <div class="kanban-detail-header">
              <h3>{detailCardId ? '编辑卡片' : '添加卡片'}</h3>
              <button class="kanban-detail-close" onClick={closeDetailPanel}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
              </button>
            </div>
            <div class="kanban-detail-body">
              <div class="kanban-detail-title-field">
                <input type="text" value={detailTitle} onInput={(e) => setDetailTitle((e.target as HTMLInputElement).value)} placeholder="输入卡片标题" />
              </div>
              <div class="kanban-detail-desc-field">
                <textarea value={detailDesc} onInput={(e) => setDetailDesc((e.target as HTMLTextAreaElement).value)} placeholder="添加描述..."></textarea>
              </div>
              <div class="kanban-detail-props" style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                {/* Priority */}
                <div style={{ display: 'flex', alignItems: 'flex-start', gap: '12px', padding: '4px 8px', borderRadius: '6px' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '13px', fontWeight: 500, color: 'var(--color-text-muted)', minWidth: 80, flexShrink: 0, paddingTop: 2 }}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style={{ width: 14, height: 14 }}><path d="M4 15s1-1 4-1 5 2 8 2 4-1 4-1V3s-1 1-4 1-5-2-8-2-4 1-4 1z"/><line x1="4" y1="22" x2="4" y2="15"/></svg>
                    <span>优先级</span>
                  </div>
                  <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap' }}>
                    {[['high', '高'], ['medium', '中'], ['low', '低'], ['', '无']].map(([p, label]) => (
                      <button key={p} style={{
                        padding: '2px 10px', border: '1px solid transparent', borderRadius: '4px',
                        cursor: 'pointer', fontSize: '12px', fontWeight: 600,
                        color: detailPriority === p ? (p === 'high' ? '#EF4444' : p === 'medium' ? '#F59E0B' : p === 'low' ? '#3B82F6' : 'var(--color-text-muted)') : 'var(--color-text-muted)',
                        background: detailPriority === p ? (p === 'high' ? '#FEE2E2' : p === 'medium' ? '#FEF3C7' : p === 'low' ? '#DBEAFE' : 'transparent') : 'transparent',
                      }}
                        onClick={() => setDetailPriority(p || null)}>{label}</button>
                    ))}
                  </div>
                </div>
                {/* Due Date */}
                <div style={{ display: 'flex', alignItems: 'center', gap: '12px', padding: '4px 8px', borderRadius: '6px' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '13px', fontWeight: 500, color: 'var(--color-text-muted)', minWidth: 80, flexShrink: 0, paddingTop: 2 }}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style={{ width: 14, height: 14 }}><rect x="3" y="4" width="18" height="18" rx="2" ry="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/></svg>
                    <span>截止日期</span>
                  </div>
                  <input type="date" value={detailDueDate} onInput={(e) => setDetailDueDate((e.target as HTMLInputElement).value)}
                    style={{ flex: 1, maxWidth: 180, padding: '2px 6px', border: '1px solid var(--color-border)', borderRadius: '4px', fontSize: '13px', color: 'var(--color-text-secondary)', background: 'transparent', outline: 'none', fontFamily: 'inherit', cursor: 'pointer' }} />
                </div>
                {/* Tags */}
                <div style={{ display: 'flex', alignItems: 'flex-start', gap: '12px', padding: '4px 8px', borderRadius: '6px' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '13px', fontWeight: 500, color: 'var(--color-text-muted)', minWidth: 80, flexShrink: 0, paddingTop: 2 }}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style={{ width: 14, height: 14 }}><path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"/><line x1="7" y1="7" x2="7.01" y2="7"/></svg>
                    <span>标签</span>
                  </div>
                  <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '4px' }}>
                    <div style={{ display: 'flex', flexWrap: 'wrap', gap: '4px', alignItems: 'center' }}>
                      {detailTags.map(t => (
                        <span key={t.id} style={{ display: 'inline-flex', alignItems: 'center', padding: '1px 6px', borderRadius: 3, fontSize: 11, fontWeight: 600, whiteSpace: 'nowrap', background: t.color + '18', color: t.color }}>
                          {t.name}
                          <span style={{ display: 'inline-flex', alignItems: 'center', marginLeft: 3, cursor: 'pointer', opacity: 0.6, fontSize: 14, lineHeight: 1 }} onClick={() => { setDetailTags(ts => ts.filter(x => x.id !== t.id)); }}>&times;</span>
                        </span>
                      ))}
                    </div>
                    <div style={{ position: 'relative' }}>
                      <button style={{ display: 'inline-flex', alignItems: 'center', gap: 3, padding: '2px 6px', border: '1px dashed var(--color-border)', borderRadius: 3, background: 'transparent', color: 'var(--color-text-muted)', fontSize: 12, cursor: 'pointer' }}
                        onClick={(e) => { e.stopPropagation(); setTagSelectorOpen(o => !o); }}>
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style={{ width: 11, height: 11 }}><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
                        添加
                      </button>
                      {tagSelectorOpen && (
                        <div style={{ position: 'absolute', top: '100%', left: 0, zIndex: 10, background: 'var(--color-elevated)', border: '1px solid var(--color-border)', borderRadius: 6, boxShadow: 'var(--shadow-8)', padding: 4, minWidth: 180 }}
                          onClick={(e) => e.stopPropagation()}>
                          {(() => {
                            const avail = tags.filter(t => !detailTags.some(d => d.id === t.id));
                            return avail.length > 0 ? avail.map(t => (
                              <div key={t.id} style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 8px', borderRadius: 4, cursor: 'pointer', fontSize: 13, color: 'var(--color-text-secondary)' }}
                                onClick={() => { setDetailTags(ts => [...ts, { ...t }]); setTagSelectorOpen(false); }}>
                                <span style={{ width: 8, height: 8, borderRadius: 2, flexShrink: 0, background: t.color }}></span>{t.name}
                              </div>
                            )) : <div style={{ padding: 8, color: '#9CA3AF', fontSize: 13 }}>暂无可用标签</div>;
                          })()}
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              </div>
              {/* Subtasks */}
              <div style={{ display: 'flex', alignItems: 'flex-start', gap: '12px', padding: '4px 8px', borderRadius: '6px' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '13px', fontWeight: 500, color: 'var(--color-text-muted)', minWidth: 80, flexShrink: 0, paddingTop: 2 }}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style={{ width: 14, height: 14 }}><polyline points="9 11 12 14 22 4"/><path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11"/></svg>
                  <span>子任务</span>
                </div>
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '2px' }}>
                  {(() => {
                    const undone = detailSubtasks.filter(s => !s.done);
                    const done = detailSubtasks.filter(s => s.done);
                    return (
                      <>
                        {undone.map(s => (
                          <div key={s.id} class="kanban-subtask-item">
                            <label class="kanban-subtask-label">
                              <input type="checkbox" checked={s.done} onChange={() => toggleSubtask(s.id)} />
                              <span>{s.title}</span>
                            </label>
                            <button class="kanban-subtask-delete" onClick={() => deleteSubtask(s.id)} title="删除">
                              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style={{ width: 12, height: 12 }}><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                            </button>
                          </div>
                        ))}
                        {done.map(s => (
                          <div key={s.id} class="kanban-subtask-item">
                            <label class="kanban-subtask-label done">
                              <input type="checkbox" checked={s.done} onChange={() => toggleSubtask(s.id)} />
                              <span>{s.title}</span>
                            </label>
                            <button class="kanban-subtask-delete" onClick={() => deleteSubtask(s.id)} title="删除">
                              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style={{ width: 12, height: 12 }}><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                            </button>
                          </div>
                        ))}
                      </>
                    );
                  })()}
                  <div class="kanban-subtask-add">
                    <textarea
                      value={subtaskInput}
                      onInput={(e) => setSubtaskInput((e.target as HTMLTextAreaElement).value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); addSubtask(); }
                      }}
                      placeholder="添加子任务..."
                      rows={1}
                    />
                    <span class="kanban-subtask-hint">Enter 添加 · Shift+Enter 换行</span>
                  </div>
                </div>
              </div>
            </div>
            <div class="kanban-detail-footer">
              {detailCardId && (
                <button class="kanban-btn-danger" onClick={deleteCardFromPanel}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style={{ width: 14, height: 14 }}><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
                  删除
                </button>
              )}
              <div style={{ marginLeft: 'auto', display: 'flex', gap: '8px' }}>
                <button class="kanban-btn-secondary" onClick={closeDetailPanel}>关闭</button>
              </div>
            </div>
          </div>
        </>
      )}

      {/* Column Modal */}
      {columnModalOpen && (
        <div class="kanban-modal-overlay" onClick={closeColumnModal}>
          <div class="kanban-modal" onClick={e => e.stopPropagation()}>
            <div class="kanban-modal-header">
              <h3>{editingColumnId ? '编辑列' : '添加列'}</h3>
              <button class="kanban-modal-close" onClick={closeColumnModal}>&times;</button>
            </div>
            <div class="kanban-modal-form">
              <div class="kanban-form-group">
                <label>列名称 *</label>
                <input type="text" value={columnFormTitle} onInput={(e) => handleColumnTitleInput((e.target as HTMLInputElement).value)}
                  onKeyDown={(e) => { if (e.key === 'Enter') { e.preventDefault(); closeColumnModal(); } }}
                  placeholder="输入列名称" />
              </div>
              <div class="kanban-modal-footer">
                <button class="kanban-btn-secondary" onClick={closeColumnModal}>关闭</button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Delete Confirmation Modal */}
      {deleteModalOpen && (
        <div class="kanban-modal-overlay" onClick={() => setDeleteModalOpen(false)}>
          <div class="kanban-modal" onClick={e => e.stopPropagation()} style={{ maxWidth: 420 }}>
            <div class="kanban-modal-header">
              <h3>确认删除</h3>
              <button class="kanban-modal-close" onClick={() => setDeleteModalOpen(false)}>&times;</button>
            </div>
            <p style={{ color: '#4B5563', fontSize: 14, lineHeight: 1.6 }}>{deleteMessage}</p>
            <div class="kanban-modal-footer">
              <button class="kanban-btn-secondary" onClick={() => setDeleteModalOpen(false)}>取消</button>
              <button class="kanban-btn-danger" onClick={async () => { if (deleteCallback) await deleteCallback(); setDeleteModalOpen(false); }}>删除</button>
            </div>
          </div>
        </div>
      )}

      {/* Settings Modal */}
      {settingsOpen && (
        <div class="kanban-modal-overlay" onClick={() => setSettingsOpen(false)}>
          <div class="kanban-modal" onClick={e => e.stopPropagation()} style={{ maxWidth: 560 }}>
            <div class="kanban-modal-header">
              <h3>看板设置</h3>
              <button class="kanban-modal-close" onClick={() => setSettingsOpen(false)}>&times;</button>
            </div>
            <div class="kanban-settings-section">
              <div class="kanban-form-group">
                <label>列顺序调整</label>
                <p style={{ color: '#9CA3AF', fontSize: 12, marginBottom: 16 }}>使用上下箭头调整看板列的显示顺序</p>
                <div class="kanban-col-order-list">
                  {cachedColumns.map((col, i) => (
                    <div key={col.id} class="kanban-col-order-item">
                      <div class="kanban-col-order-name">
                        <span class="kanban-col-order-index">{i + 1}</span>
                        <span>{col.title}</span>
                      </div>
                      <div class="kanban-col-order-btns">
                        <button class="kanban-order-btn" disabled={i === 0} onClick={() => handleMoveCol(col.id, 'up')}>
                          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="18 15 12 9 6 15"/></svg>
                        </button>
                        <button class="kanban-order-btn" disabled={i === cachedColumns.length - 1} onClick={() => handleMoveCol(col.id, 'down')}>
                          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </div>
            <div class="kanban-settings-section">
              <div class="kanban-form-group">
                <label>标签管理</label>
                <p style={{ color: '#9CA3AF', fontSize: 12, marginBottom: 16 }}>添加或删除可用标签</p>
                <div class="kanban-tag-manager" style={{ marginBottom: 12 }}>
                  {settingsTags.map(t => (
                    <span key={t.id} class="kanban-tag" style={{ background: t.color + '18', color: t.color }}>
                      {t.name}
                      <span class="kanban-tag-remove" onClick={() => deleteTag(t.id)}>&times;</span>
                    </span>
                  ))}
                </div>
                <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                  <input type="text" value={newTagName} onInput={(e) => setNewTagName((e.target as HTMLInputElement).value)}
                    placeholder="标签名称" class="kanban-input" style={{ flex: 1 }} />
                  <input type="color" value={newTagColor} onInput={(e) => setNewTagColor((e.target as HTMLInputElement).value)}
                    style={{ width: 32, height: 32, border: '1px solid #E5E7EB', borderRadius: 6, cursor: 'pointer', padding: 2 }} />
                  <button class="kanban-btn-primary" style={{ padding: '6px 14px', fontSize: 13 }} onClick={addNewTag}>添加</button>
                </div>
              </div>
            </div>
            <div class="kanban-modal-footer">
              <button class="kanban-btn-primary" onClick={() => setSettingsOpen(false)}>关闭</button>
            </div>
          </div>
        </div>
      )}

      {/* Toast */}
      {toastMsg && <div key={toastMsg.key} class={`kanban-toast ${toastMsg.type}`}>{toastMsg.msg}</div>}
    </div>
  );
}


