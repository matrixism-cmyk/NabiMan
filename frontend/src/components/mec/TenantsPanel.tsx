import React, { useState } from 'react';
import { useT } from '../../i18n';
import { useMecList, mecGet, mecDelete } from '../../hooks/mec/useMecApi';
import { Tenant } from '../../types/mec';
import TenantWizard from './wizard/TenantWizard';
import TenantDetail from './TenantDetail';
import {
  ErrorBanner,
  PanelLayout,
  SortableTable,
  StatusBadge,
  Toolbar,
} from './common';

interface DeletePreflight {
  tenant_id: string;
  namespace_exists: boolean;
  running_pods: number;
  lb_services: number;
  has_starter_kit: boolean;
  rancher_project_exists: boolean;
  dedicated_node?: string | null;
  warnings: string[];
  confirm_token: string;
}

function tenantStatusLabel(t: Tenant): string {
  const s = t.status;
  if ('state' in s) {
    return s.state;
  }
  return 'unknown';
}

export default function TenantsPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useMecList<Tenant>(
    '/api/mec/v1/tenants',
    30_000,
  );
  const [showNew, setShowNew] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [filter, setFilter] = useState('');

  if (selectedId) {
    return (
      <TenantDetail
        tenantId={selectedId}
        onClose={() => {
          setSelectedId(null);
          refetch();
        }}
      />
    );
  }

  const handleDelete = async (id: string) => {
    setBusyId(id);
    setActionError(null);
    try {
      const pf = await mecGet<DeletePreflight>(
        `/api/mec/v1/tenants/${encodeURIComponent(id)}/delete-preflight`,
      );
      if (pf.error) {
        setActionError(pf.error.message);
        return;
      }
      const info = pf.data;
      if (!info) {
        setActionError(t('mec.tenant.delete.emptyResponse'));
        return;
      }

      const summary = [
        t('mec.tenant.delete.heading', { id }),
        '',
        t('mec.tenant.delete.runningPods', { count: info.running_pods }),
        t('mec.tenant.delete.lbServices', { count: info.lb_services }),
        t('mec.tenant.delete.starterKit', {
          value: info.has_starter_kit ? t('mec.tenant.present') : t('mec.tenant.absent'),
        }),
        t('mec.tenant.delete.rancherProject', {
          value: info.rancher_project_exists
            ? t('mec.tenant.presentDeleted')
            : t('mec.tenant.absent'),
        }),
        info.dedicated_node
          ? t('mec.tenant.delete.dedicatedNode', { node: info.dedicated_node })
          : '',
        '',
        ...info.warnings.map((w) => `⚠️ ${w}`),
        '',
        t('mec.tenant.delete.confirmPrompt'),
      ]
        .filter(Boolean)
        .join('\n');

      const typed = window.prompt(summary, '');
      if (typed !== id) {
        if (typed !== null) {
          setActionError(t('mec.tenant.delete.mismatch'));
        }
        return;
      }
      await mecDelete(`/api/mec/v1/tenants/${encodeURIComponent(id)}`, id);
      await refetch();
    } catch (e) {
      setActionError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyId(null);
    }
  };

  const handleCreated = async () => {
    setShowNew(false);
    await refetch();
  };

  const downloadGuide = (id: string, format: 'txt' | 'docx') => {
    const token = sessionStorage.getItem('nabiman_token');
    const url = `/api/mec/v1/tenants/${encodeURIComponent(id)}/guide.${format}`;
    fetch(url, {
      headers: token ? { Authorization: `Bearer ${token}` } : {},
    })
      .then(async (r) => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        const blob = await r.blob();
        const a = document.createElement('a');
        a.href = URL.createObjectURL(blob);
        a.download = `access_guide_${id}.${format}`;
        a.click();
        URL.revokeObjectURL(a.href);
      })
      .catch((e) => setActionError(String(e)));
  };

  const tenants = data || [];
  const active = tenants.filter((t) => tenantStatusLabel(t) === 'active').length;

  return (
    <PanelLayout
      title={t('mec.tenant.title')}
      subtitle={t('mec.tenant.subtitle', { count: tenants.length, active })}
      actions={
        <>
          <button className="btn btn-secondary" onClick={refetch}>
            {t('mec.action.refresh')}
          </button>
          <button
            className="btn btn-primary"
            onClick={() => setShowNew((v) => !v)}
          >
            {showNew ? t('mec.tenant.cancel') : t('mec.tenant.new')}
          </button>
        </>
      }
    >
      <ErrorBanner error={error || undefined} />
      <ErrorBanner error={actionError || undefined} />

      {showNew && (
        <div style={{ marginBottom: '16px' }}>
          <TenantWizard
            onCreated={handleCreated}
            onCancel={() => setShowNew(false)}
          />
        </div>
      )}

      <Toolbar marginBottom="12px">
        <input
          placeholder={t('mec.tenant.filter')}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          style={{ padding: '6px 10px', minWidth: '240px' }}
        />
      </Toolbar>

      {loading && !data && <div style={{ color: '#6b7280' }}>{t('mec.state.loading')}</div>}

      <SortableTable<Tenant>
        data={tenants}
        filter={filter}
        defaultSortKey="id"
        rowKey={(row) => row.id}
        onRowClick={(row) => setSelectedId(row.id)}
        emptyMessage={t('mec.tenant.empty')}
        columns={[
          {
            key: 'id',
            header: t('mec.tenant.col.id'),
            accessor: (row) => row.id,
            render: (row) => <code>{row.id}</code>,
            sortable: true,
          },
          {
            key: 'display_name',
            header: t('mec.tenant.col.company'),
            accessor: (row) => row.display_name,
            sortable: true,
          },
          {
            key: 'namespace',
            header: t('mec.tenant.col.namespace'),
            accessor: (row) => row.namespace,
            render: (row) => (
              <span style={{ color: '#6b7280' }}>{row.namespace}</span>
            ),
            sortable: true,
          },
          {
            key: 'node',
            header: t('mec.tenant.col.node'),
            accessor: (row) => row.allocation.node || '',
            render: (row) =>
              row.allocation.node ? (
                <StatusBadge tone="info">{row.allocation.node}</StatusBadge>
              ) : (
                <span style={{ color: '#9ca3af' }}>{t('mec.tenant.shared')}</span>
              ),
            sortable: true,
          },
          {
            key: 'gpu',
            header: 'GPU',
            accessor: (row) => row.quota.gpu,
            render: (row) =>
              row.quota.gpu > 0 ? (
                <span>
                  <span style={{ color: '#6b7280' }}>
                    {row.allocation.gpu_label || ''}
                  </span>{' '}
                  × <strong>{row.quota.gpu}</strong>
                </span>
              ) : (
                <span style={{ color: '#d1d5db' }}>-</span>
              ),
            align: 'right',
            sortable: true,
          },
          {
            key: 'cpu',
            header: 'CPU (req/lim)',
            accessor: (row) => parseFloat(row.quota.cpu_requests) || 0,
            render: (row) => `${row.quota.cpu_requests} / ${row.quota.cpu_limits}`,
            align: 'right',
            sortable: true,
          },
          {
            key: 'mem',
            header: 'Memory (req/lim)',
            accessor: (row) => row.quota.memory_requests,
            render: (row) =>
              `${row.quota.memory_requests} / ${row.quota.memory_limits}`,
            align: 'right',
            sortable: true,
          },
          {
            key: 'status',
            header: t('mec.tenant.col.status'),
            accessor: (row) => tenantStatusLabel(row),
            render: (row) => (
              <StatusBadge tone={tenantStatusLabel(row)}>
                {tenantStatusLabel(row)}
              </StatusBadge>
            ),
            width: '100px',
            sortable: true,
          },
          {
            key: 'actions',
            header: '',
            accessor: () => '',
            sortable: false,
            width: '280px',
            render: (row) => (
              <div style={{ display: 'flex', gap: '4px' }}>
                <button
                  className="btn btn-primary btn-small"
                  onClick={(e) => {
                    e.stopPropagation();
                    setSelectedId(row.id);
                  }}
                >
                  {t('mec.tenant.action.detail')}
                </button>
                <button
                  className="btn btn-secondary btn-small"
                  onClick={(e) => {
                    e.stopPropagation();
                    downloadGuide(row.id, 'docx');
                  }}
                  title={t('mec.tenant.title.docx')}
                >
                  docx
                </button>
                <button
                  className="btn btn-secondary btn-small"
                  onClick={(e) => {
                    e.stopPropagation();
                    downloadGuide(row.id, 'txt');
                  }}
                  title={t('mec.tenant.title.txt')}
                >
                  txt
                </button>
                <button
                  className="btn btn-danger btn-small"
                  disabled={busyId === row.id}
                  onClick={(e) => {
                    e.stopPropagation();
                    handleDelete(row.id);
                  }}
                >
                  {busyId === row.id ? '...' : t('mec.tenant.action.delete')}
                </button>
              </div>
            ),
          },
        ]}
      />
    </PanelLayout>
  );
}
