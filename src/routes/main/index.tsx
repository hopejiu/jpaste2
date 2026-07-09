import { FluentIcon } from '../../components/fluent-icon';
import { SearchBar } from './search-bar';
import { EntryList } from './entry-list';
import { Modal } from '../../components/modal';
import { ShortcutHelp } from '../../components/shortcut-help';
import { QueuePopup } from '../../components/queue-popup';
import { FolderModal } from '../../actions/folder';
import { FileUriModal } from '../../actions/open-url';
import { TAG_TABS, TAG_FAVORITE } from '../../lib/types';
import { api } from '../../lib/invoke';
import { copyToClipboard } from '../../lib/clipboard';
import { error as logError, setComponent } from '../../lib/logger';
import { searchQuery, tagFilter, isRegex, setSearchQuery, setTagFilter, setIsRegex, deleteEntry, toggleFavorite } from '../../hooks/use-entries';
import { useMainPage } from './use-main-page';
import { useSidebarTabShortcuts } from '../../hooks/use-sidebar-tabs';

setComponent('main');

/** Get the current hash path without leading '#' or query string. */
function currentBasePath() {
  const hash = window.location.hash;
  const p = hash.startsWith('#') ? hash.slice(1) : hash;
  const quest = p.indexOf('?');
  return quest >= 0 ? p.slice(0, quest) : p;
}

export function MainPage() {
  const {
    inputRef,
    focusedIndex, setFocusedIndex,
    showHelp, setShowHelp,
    errorAlert, setErrorAlert,
    pinned, setPinned,
    qrModal, qrText, qrLoading, setQrModal,
    filoMode, queueItems, refreshFiloItems,
    sortField, sortOrder,
    currentEntries, hasMoreVal, loadingVal, loadMore,
    handleSelect, handleImageClick, handleQrClick, handleCopyQr,
    handleActionClick, handleOpenEditorById,
    handleSortChange, setPasteOrder,
  } = useMainPage();

  useSidebarTabShortcuts();

  const tagTabs = TAG_TABS;
  const basePath = currentBasePath();

  const sidebarItems = [
    { path: '/', icon: 'clipboard' as const, label: '剪贴板' },
    { path: '/toolbox', icon: 'toolbox' as const, label: '工具箱' },
  ];

  return (
    <div class="main-page">
      {/* Title Bar — also serves as sidebar nav */}
      <div class="title-bar" data-tauri-drag-region>
        <div class="title-left">
          {sidebarItems.map((item) => (
            <button
              key={item.path}
              class={`sidebar-btn ${basePath === item.path ? 'active' : ''}`}
              title={item.label}
              onClick={() => window.location.hash = item.path || '/'}
              aria-label={item.label}
            >
              <FluentIcon name={item.icon} size={18} />
            </button>
          ))}
        </div>
        <span class="title-text">jPaste</span>
        <div class="title-right">
          <button class="title-btn" title="设置" onClick={() => window.location.hash = '/settings'} aria-label="设置">
            <FluentIcon name="settings" size={18} />
          </button>
          <button
            class={`title-btn ${pinned ? 'active' : ''}`}
            title={pinned ? '取消置顶' : '置顶窗口'}
            onClick={async () => {
              try {
                const newPinned = await api.togglePinned();
                setPinned(newPinned);
              } catch (e) { logError('toggle pinned', e); }}
            }
            aria-label={pinned ? '取消置顶' : '置顶窗口'}
          >
            <FluentIcon name="pin" size={18} filled={pinned} />
          </button>
        </div>
      </div>

      {/* Search Header */}
      <div class="search-header">
        <SearchBar
          value={searchQuery.value}
          inputRef={inputRef}
          onSearch={(v) => { setSearchQuery(v); }}
          sortField={sortField}
          sortOrder={sortOrder}
          onSortChange={handleSortChange}
        />
        <button
          class={`regex-toggle ${isRegex.value ? 'active' : ''}`}
          onClick={() => { setIsRegex(!isRegex.value); }}
          title="正则搜索"
        >.*</button>
      </div>

      {/* Tag Tabs */}
      <div class="tag-tabs-bar">
        {tagTabs.map((tab) => (
          <button
            key={tab.mask}
            class={`tag-tab ${tagFilter.value === tab.mask ? 'active' : ''}`}
            onClick={() => { setTagFilter(tab.mask); setFocusedIndex(-1); }}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Copy All (only in favorites view) */}
      {tagFilter.value === TAG_FAVORITE && currentEntries.length > 0 && (
        <div class="copy-all-bar">
          <button class="copy-all-btn" onClick={() => {
            const text = currentEntries.map(e => e.content).filter(Boolean).join('\n');
            if (text) copyToClipboard(text);
          }}>
            <FluentIcon name="copy" size={16} /> 复制所有
          </button>
        </div>
      )}

      {/* Entry List — flex-1 fills remaining space */}
      <EntryList
        entries={currentEntries}
        hasMore={hasMoreVal}
        loading={loadingVal}
        focusedIndex={focusedIndex}
        onLoadMore={loadMore}
        onFocus={setFocusedIndex}
        onSelect={handleSelect}
        onDelete={async (id) => { await deleteEntry(id); }}
        onToggleFav={async (id, val) => { await toggleFavorite(id, val); }}
        onImageClick={handleImageClick}
        onActionClick={handleActionClick}
        onQrClick={handleQrClick}
        onOpenEditor={handleOpenEditorById}
      />

      {/* Footer / Bottom Bar */}
      <div class="bottom-bar">
        <div class="bottom-left">
          <button class="help-btn" onClick={() => setShowHelp(true)} title="快捷键说明" aria-label="快捷键说明">
            <FluentIcon name="help" size={18} />
          </button>
          <span class="bottom-hints">Ctrl+L搜索 · E编辑 · C复制 · Del删除 · Space收藏 · [ ]切换 · Esc隐藏</span>
        </div>
        <QueuePopup
          mode={filoMode.value}
          items={queueItems.value}
          onModeChange={setPasteOrder}
          onRefreshItems={refreshFiloItems}
        />
      </div>

      {/* Shortcut Help Modal */}
      <ShortcutHelp open={showHelp} onClose={() => setShowHelp(false)} />

      {/* Error Alert Modal */}
      <Modal open={!!errorAlert} onClose={() => setErrorAlert(null)} title={errorAlert?.title ?? ''}>
        <p class="error-msg">{errorAlert?.message}</p>
        <button class="error-btn" onClick={() => setErrorAlert(null)}>确定</button>
      </Modal>

      {/* Folder/File choice modal */}
      <FolderModal />

      {/* file:// URI choice modal */}
      <FileUriModal />

      {/* QR Code Content Modal */}
      <Modal
        open={!!qrModal}
        onClose={() => setQrModal(null)}
        title={qrModal ? '二维码内容' : ''}
      >
        {qrLoading ? (
          <p class="qr-modal-loading">正在解码二维码...</p>
        ) : (
          <>
            <div class="qr-modal-text">{qrText}</div>
            <div class="qr-modal-actions">
              <button
                class="viewer-btn primary"
                onClick={() => handleCopyQr(qrText)}
                disabled={!qrText || qrText === '未找到二维码内容' || qrText === '二维码解析失败'}
              >
                <FluentIcon name="copy" size={16} /> 复制二维码
              </button>
            </div>
          </>
        )}
      </Modal>

    </div>
  );
}
