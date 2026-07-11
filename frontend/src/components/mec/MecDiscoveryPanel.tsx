import React, { useState } from 'react';
import { useT } from '../../i18n';
import { useMecList, mecPost } from '../../hooks/mec/useMecApi';
import {
  ErrorBanner,
  PanelLayout,
  SortableTable,
  StatusBadge,
  Toolbar,
} from './common';

interface DiscoveredTenant {
  id: string;
  namespace: string;
  labels: Record<string, string>;
  already_managed: boolean;
  suspected_tenant: boolean;
}

export default function MecDiscoveryPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useMecList<DiscoveredTenant>(
    '/api/mec/v1/tenants/discover',
    60_000,
  );
  const [busyId, setBusyId] = useState<string | null>(null);
  const [actionErr, setActionErr] = useState<string | null>(null);
  const [filter, setFilter] = useState('');

  const importOne = async (id: string) => {
    setBusyId(id);
    setActionErr(null);
    try {
      const res = await mecPost(
        `/api/mec/v1/tenants/${encodeURIComponent(id)}/import`,
        {},
      );
      if (res.error) setActionErr(res.error.message);
      else await refetch();
    } catch (e) {
      setActionErr(String(e));
    } finally {
      setBusyId(null);
    }
  };

  const rows = data || [];
  const unmanaged = rows.filter((r) => !r.already_managed).length;

  return (
    <PanelLayout
      title={t('mec.disc.title')}
      subtitle={t('mec.disc.subtitle', { found: rows.length, unmanaged })}
      actions={
        <button className="btn btn-secondary" onClick={refetch}>
          {t('mec.disc.rescan')}
        </button>
      }
    >
      <ErrorBanner error={error || undefined} />
      <ErrorBanner error={actionErr || undefined} />

      <div
        style={{
          background: '#f0f9ff',
          borderLeft: '3px solid #3b82f6',
          color: '#1e40af',
          padding: '10px 14px',
          borderRadius: '6px',
          fontSize: '13px',
          marginBottom: '12px',
        }}
      >
        ℹ️ <strong>Discovery</strong>{t('mec.disc.infoIsProbe')}{' '}
        <code>-poc/-dev/-prod</code>{t('mec.disc.infoSuffix')}{' '}
        <code>tenant</code>{t('mec.disc.infoLabel')}
      </div>

      <Toolbar>
        <input
          placeholder={t('mec.disc.filterPlaceholder')}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          style={{ padding: '6px 10px', minWidth: '240px' }}
        />
      </Toolbar>

      {loading && !data && <div style={{ color: '#6b7280' }}>{t('mec.disc.scanning')}</div>}

      <SortableTable<DiscoveredTenant>
        data={rows}
        filter={filter}
        defaultSortKey="namespace"
        rowKey={(r) => r.id}
        emptyMessage={t('mec.disc.noResults')}
        columns={[
          {
            key: 'namespace',
            header: t('mec.disc.colNamespace'),
            accessor: (r) => r.namespace,
            render: (r) => <code>{r.namespace}</code>,
            sortable: true,
          },
          {
            key: 'labels',
            header: t('mec.disc.colLabels'),
            accessor: (r) =>
              Object.entries(r.labels)
                .slice(0, 4)
                .map(([k, v]) => `${k}=${v}`)
                .join(','),
            render: (r) => (
              <span style={{ fontSize: '11px', fontFamily: 'monospace' }}>
                {Object.entries(r.labels)
                  .filter(
                    ([k]) =>
                      !k.startsWith('kubernetes.io/') &&
                      !k.startsWith('app.kubernetes.io/'),
                  )
                  .slice(0, 4)
                  .map(([k, v]) => `${k}=${v}`)
                  .join(', ') || <span style={{ color: '#d1d5db' }}>-</span>}
              </span>
            ),
            sortable: true,
          },
          {
            key: 'status',
            header: t('mec.disc.colStatus'),
            accessor: (r) => (r.already_managed ? 'imported' : 'unmanaged'),
            render: (r) => (
              <StatusBadge tone={r.already_managed ? 'success' : 'warning'}>
                {r.already_managed ? 'imported' : 'unmanaged'}
              </StatusBadge>
            ),
            width: '120px',
            sortable: true,
          },
          {
            key: 'actions',
            header: '',
            accessor: () => '',
            sortable: false,
            width: '120px',
            render: (r) =>
              r.already_managed ? (
                <span style={{ color: '#9ca3af', fontSize: '12px' }}>
                  {t('mec.disc.registered')}
                </span>
              ) : (
                <button
                  className="btn btn-primary btn-small"
                  disabled={busyId === r.id}
                  onClick={() => importOne(r.id)}
                >
                  {busyId === r.id ? t('mec.disc.importing') : 'Import'}
                </button>
              ),
          },
        ]}
      />
    </PanelLayout>
  );
}
