import React, { useState } from 'react';
import { useT } from '../../i18n';
import { useMecList } from '../../hooks/mec/useMecApi';
import { AuditLog } from '../../types/mec';
import { ErrorBanner, PanelLayout, SortableTable, StatusBadge, Toolbar } from './common';

export default function MecAuditPanel() {
  const { t } = useT();
  const [filter, setFilter] = useState('');
  const { data, loading, error, refetch } = useMecList<AuditLog>(
    '/api/mec/v1/audit/logs?limit=200',
    60_000,
  );

  const logs = data || [];
  const successCount = logs.filter((l) => l.status === 'success').length;
  const failedCount = logs.filter((l) => l.status === 'failed').length;

  return (
    <PanelLayout
      title={t('mec.audit.title')}
      subtitle={t('mec.audit.subtitle', { total: logs.length, success: successCount, failed: failedCount })}
      actions={
        <button className="btn btn-secondary" onClick={refetch}>
          {t('mec.action.refresh')}
        </button>
      }
    >
      <ErrorBanner error={error || undefined} />

      <Toolbar>
        <input
          placeholder={t('mec.audit.filterPlaceholder')}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          style={{ padding: '6px 10px', minWidth: '280px' }}
        />
      </Toolbar>

      {loading && !data && <div style={{ color: '#6b7280' }}>{t('mec.state.loading')}</div>}

      <SortableTable<AuditLog>
        data={logs}
        filter={filter}
        defaultSortKey="timestamp"
        defaultSortDir="desc"
        rowKey={(r) => r.id}
        emptyMessage={t('mec.audit.empty')}
        columns={[
          {
            key: 'timestamp',
            header: t('mec.col.time'),
            accessor: (r) => r.timestamp,
            render: (r) => new Date(r.timestamp).toLocaleString(),
            width: '170px',
            sortable: true,
          },
          {
            key: 'user',
            header: t('mec.audit.colUser'),
            accessor: (r) => r.user,
            width: '100px',
            sortable: true,
          },
          {
            key: 'action',
            header: t('mec.audit.colAction'),
            accessor: (r) => r.action,
            render: (r) => <code style={{ fontSize: '12px' }}>{r.action}</code>,
            sortable: true,
          },
          {
            key: 'resource',
            header: t('mec.audit.colResource'),
            accessor: (r) => `${r.resource_type}/${r.resource_id}`,
            render: (r) => (
              <>
                <span style={{ color: '#6b7280' }}>{r.resource_type}</span>
                <span style={{ color: '#d1d5db', margin: '0 4px' }}>/</span>
                <code>{r.resource_id}</code>
              </>
            ),
            sortable: true,
          },
          {
            key: 'status',
            header: t('mec.audit.colResult'),
            accessor: (r) => r.status,
            render: (r) => <StatusBadge tone={r.status}>{r.status}</StatusBadge>,
            width: '100px',
            sortable: true,
          },
          {
            key: 'duration_ms',
            header: t('mec.audit.colDuration'),
            accessor: (r) => r.duration_ms,
            render: (r) => `${r.duration_ms}ms`,
            align: 'right',
            width: '80px',
            sortable: true,
          },
          {
            key: 'ops',
            header: t('mec.audit.colOps'),
            accessor: (r) => r.operations.length,
            render: (r) =>
              r.operations.length > 0 ? (
                <details>
                  <summary style={{ cursor: 'pointer' }}>
                    {r.operations.length}
                  </summary>
                  <ul
                    style={{
                      fontFamily: 'monospace',
                      fontSize: '11px',
                      margin: '4px 0',
                      paddingLeft: '16px',
                    }}
                  >
                    {r.operations.map((op, i) => (
                      <li key={i} style={{ color: '#6b7280' }}>
                        {op.name} → {op.status} ({op.duration_ms}ms)
                      </li>
                    ))}
                  </ul>
                </details>
              ) : (
                <span style={{ color: '#d1d5db' }}>0</span>
              ),
            align: 'right',
            width: '70px',
            sortable: true,
          },
        ]}
      />
    </PanelLayout>
  );
}
