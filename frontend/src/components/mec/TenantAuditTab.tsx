import React from 'react';
import { useT } from '../../i18n';
import { useMecList } from '../../hooks/mec/useMecApi';
import { AuditLog } from '../../types/mec';
import { ErrorBanner, SortableTable, StatusBadge } from './common';

interface Props {
  tenantId: string;
}

export default function TenantAuditTab({ tenantId }: Props) {
  const { t } = useT();
  const { data, loading, error, refetch } = useMecList<AuditLog>(
    `/api/mec/v1/audit/logs?resource_type=tenant&resource_id=${encodeURIComponent(tenantId)}&limit=100`,
    30_000,
  );

  return (
    <div>
      <div
        style={{
          display: 'flex',
          justifyContent: 'flex-end',
          marginBottom: '8px',
        }}
      >
        <button className="btn btn-secondary btn-small" onClick={refetch}>
          {t('mec.action.refresh')}
        </button>
      </div>
      {loading && !data && (
        <div style={{ color: 'var(--text-secondary)' }}>{t('mec.state.loading')}</div>
      )}
      <ErrorBanner error={error || undefined} />

      <SortableTable<AuditLog>
        data={data || []}
        defaultSortKey="timestamp"
        defaultSortDir="desc"
        rowKey={(r) => r.id}
        emptyMessage={t('mec.tenant.audit.empty')}
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
            header: t('mec.tenant.audit.colUser'),
            accessor: (r) => r.user,
            width: '100px',
            sortable: true,
          },
          {
            key: 'action',
            header: t('mec.tenant.audit.colAction'),
            accessor: (r) => r.action,
            render: (r) => <code>{r.action}</code>,
            sortable: true,
          },
          {
            key: 'status',
            header: t('mec.tenant.audit.colResult'),
            accessor: (r) => r.status,
            render: (r) => <StatusBadge tone={r.status}>{r.status}</StatusBadge>,
            width: '100px',
            sortable: true,
          },
          {
            key: 'duration_ms',
            header: t('mec.tenant.audit.colDuration'),
            accessor: (r) => r.duration_ms,
            render: (r) => `${r.duration_ms}ms`,
            align: 'right',
            width: '80px',
            sortable: true,
          },
          {
            key: 'ops',
            header: 'Ops',
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
                      <li key={i} style={{ color: 'var(--text-secondary)' }}>
                        {op.name} → {op.status} ({op.duration_ms}ms)
                      </li>
                    ))}
                  </ul>
                </details>
              ) : (
                <span style={{ color: 'var(--text-secondary)' }}>0</span>
              ),
            align: 'right',
            width: '70px',
            sortable: true,
          },
        ]}
      />
    </div>
  );
}
