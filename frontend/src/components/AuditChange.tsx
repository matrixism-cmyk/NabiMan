import React from 'react';
import { useT } from '../i18n';

export interface AuditFieldChange { field: string; old?: string; new?: string; secret?: boolean; }
export interface AuditChangeRecord { resource: string; id: string; action: string; fields?: AuditFieldChange[]; }

const ACTIONS = ['create', 'update', 'delete', 'close'];

/** What an audit entry's administrative change changed: resource, id, and old → new per field. */
export default function AuditChange({ change }: { change?: AuditChangeRecord | null }) {
  const { t } = useT();
  if (!change) return null;
  const action = ACTIONS.includes(change.action) ? t(`audit.change.${change.action}`) : change.action;
  const none = t('audit.change.none');
  return (
    <div className="audit-change" style={{ fontSize: '0.85em' }}>
      <div><code>{change.resource}</code> <code>{change.id}</code> · {action}</div>
      {(change.fields || []).length > 0 && (
        <ul style={{ margin: '2px 0 0', paddingLeft: 16 }}>
          {(change.fields || []).map((f) => (
            <li key={f.field}>
              <code>{f.field}</code>:{' '}
              {f.secret ? <em>{t('audit.change.secret')}</em> : <span>{f.old ?? none} → {f.new ?? none}</span>}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
