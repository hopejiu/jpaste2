interface SearchBarProps {
  value: string;
  inputRef?: { current: HTMLInputElement | null };
  onSearch: (v: string) => void;
  sortField: string;
  sortOrder: string;
  onSortChange: (field: string, order: string) => void;
}

export function SearchBar({ value, inputRef, onSearch, sortField, sortOrder, onSortChange }: SearchBarProps) {
  return (
    <div class="search-row">
      <input
        ref={(el) => { if (inputRef) (inputRef as any).current = el; }}
        type="text"
        class="search-input"
        placeholder="搜索剪贴板历史..."
        value={value}
        onInput={(e) => onSearch((e.target as HTMLInputElement).value)}
      />
      <select
        class="sort-select"
        value={`${sortField}-${sortOrder}`}
        onChange={(e) => {
          const val = (e.target as HTMLSelectElement).value;
          const [field, order] = val.split('-');
          onSortChange(field, order);
        }}
      >
        <option value="updated_at-desc">最新</option>
        <option value="updated_at-asc">最旧</option>
        <option value="copy_count-desc">最多使用</option>
        <option value="content_length-desc">最长</option>
        <option value="content_length-asc">最短</option>
      </select>
    </div>
  );
}
