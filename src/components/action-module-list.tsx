import { useState } from 'preact/hooks';
import { FluentIcon } from './fluent-icon';
import { ACTION_ICONS } from '../routes/main/entry-item';

interface ActionModule {
  id: string;
  label: string;
  desc: string;
  trigger?: string;
}

interface ActionModuleListProps {
  modules: ActionModule[];
}

/**
 * Action module reference list — display only, no toggle/reorder.
 * Shows what content types jPaste auto-detects for notification chips.
 */
export function ActionModuleList({ modules }: ActionModuleListProps) {
  const [expandedId, setExpandedId] = useState<string | null>(null);

  return (
    <div class="action-modules-list">
      {modules.map((mod) => (
        <div class="action-module-item" key={mod.id}>
          <div class="action-module-header">
            <div class="action-module-icon">
              {ACTION_ICONS[mod.id] ? <FluentIcon name={ACTION_ICONS[mod.id]} size={18} /> : null}
            </div>
            <div class="action-module-info" onClick={() => setExpandedId(expandedId === mod.id ? null : mod.id)}>
              <div class="action-module-label">{mod.label}</div>
              <div class="action-module-desc">{mod.desc}</div>
            </div>
            <button
              class="action-module-expand"
              onClick={() => setExpandedId(expandedId === mod.id ? null : mod.id)}
              aria-label={expandedId === mod.id ? '收起' : '展开'}
            ><FluentIcon name={expandedId === mod.id ? 'subtract' : 'add'} size={16} /></button>
          </div>
          {expandedId === mod.id && (
            <div class="action-module-detail">
              <span class="action-detail-row"><span class="action-detail-label">触发：</span>{mod.trigger || mod.desc}</span>
              <span class="action-detail-row"><span class="action-detail-label">功能：</span>{mod.desc}</span>
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
