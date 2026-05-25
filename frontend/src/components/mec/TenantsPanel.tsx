import React, { useState } from 'react';
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
        setActionError('삭제 사전 검사 응답이 비어 있습니다.');
        return;
      }

      const summary = [
        `테넌트 '${id}' 완전 삭제`,
        '',
        `• 실행 중 Pod: ${info.running_pods}개`,
        `• LoadBalancer 서비스: ${info.lb_services}개`,
        `• Starter Kit: ${info.has_starter_kit ? '있음' : '없음'}`,
        `• Rancher Project: ${info.rancher_project_exists ? '있음 (삭제됨)' : '없음'}`,
        info.dedicated_node ? `• 단독 노드: ${info.dedicated_node}` : '',
        '',
        ...info.warnings.map((w) => `⚠️ ${w}`),
        '',
        '확인을 위해 테넌트 ID를 그대로 입력하세요:',
      ]
        .filter(Boolean)
        .join('\n');

      const typed = window.prompt(summary, '');
      if (typed !== id) {
        if (typed !== null) {
          setActionError('입력한 이름이 테넌트 ID와 일치하지 않아 삭제를 취소했습니다.');
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
      title="테넌트"
      subtitle={`${tenants.length}개 등록 · 활성 ${active} · Discovery 탭에서 미등록 테넌트 Import 가능`}
      actions={
        <>
          <button className="btn btn-secondary" onClick={refetch}>
            새로고침
          </button>
          <button
            className="btn btn-primary"
            onClick={() => setShowNew((v) => !v)}
          >
            {showNew ? '취소' : '+ 신규 테넌트'}
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
          placeholder="필터 (ID, 기업명, 노드, GPU)"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          style={{ padding: '6px 10px', minWidth: '240px' }}
        />
      </Toolbar>

      {loading && !data && <div style={{ color: '#6b7280' }}>로딩 중...</div>}

      <SortableTable<Tenant>
        data={tenants}
        filter={filter}
        defaultSortKey="id"
        rowKey={(t) => t.id}
        onRowClick={(t) => setSelectedId(t.id)}
        emptyMessage='등록된 테넌트가 없습니다. "+ 신규 테넌트" 또는 Discovery 탭에서 Import.'
        columns={[
          {
            key: 'id',
            header: '테넌트 ID',
            accessor: (t) => t.id,
            render: (t) => <code>{t.id}</code>,
            sortable: true,
          },
          {
            key: 'display_name',
            header: '기업명',
            accessor: (t) => t.display_name,
            sortable: true,
          },
          {
            key: 'namespace',
            header: '네임스페이스',
            accessor: (t) => t.namespace,
            render: (t) => (
              <span style={{ color: '#6b7280' }}>{t.namespace}</span>
            ),
            sortable: true,
          },
          {
            key: 'node',
            header: '노드',
            accessor: (t) => t.allocation.node || '',
            render: (t) =>
              t.allocation.node ? (
                <StatusBadge tone="info">{t.allocation.node}</StatusBadge>
              ) : (
                <span style={{ color: '#9ca3af' }}>공유</span>
              ),
            sortable: true,
          },
          {
            key: 'gpu',
            header: 'GPU',
            accessor: (t) => t.quota.gpu,
            render: (t) =>
              t.quota.gpu > 0 ? (
                <span>
                  <span style={{ color: '#6b7280' }}>
                    {t.allocation.gpu_label || ''}
                  </span>{' '}
                  × <strong>{t.quota.gpu}</strong>
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
            accessor: (t) => parseFloat(t.quota.cpu_requests) || 0,
            render: (t) => `${t.quota.cpu_requests} / ${t.quota.cpu_limits}`,
            align: 'right',
            sortable: true,
          },
          {
            key: 'mem',
            header: 'Memory (req/lim)',
            accessor: (t) => t.quota.memory_requests,
            render: (t) =>
              `${t.quota.memory_requests} / ${t.quota.memory_limits}`,
            align: 'right',
            sortable: true,
          },
          {
            key: 'status',
            header: '상태',
            accessor: (t) => tenantStatusLabel(t),
            render: (t) => (
              <StatusBadge tone={tenantStatusLabel(t)}>
                {tenantStatusLabel(t)}
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
            render: (t) => (
              <div style={{ display: 'flex', gap: '4px' }}>
                <button
                  className="btn btn-primary btn-small"
                  onClick={(e) => {
                    e.stopPropagation();
                    setSelectedId(t.id);
                  }}
                >
                  상세
                </button>
                <button
                  className="btn btn-secondary btn-small"
                  onClick={(e) => {
                    e.stopPropagation();
                    downloadGuide(t.id, 'docx');
                  }}
                  title="Word 문서"
                >
                  docx
                </button>
                <button
                  className="btn btn-secondary btn-small"
                  onClick={(e) => {
                    e.stopPropagation();
                    downloadGuide(t.id, 'txt');
                  }}
                  title="텍스트"
                >
                  txt
                </button>
                <button
                  className="btn btn-danger btn-small"
                  disabled={busyId === t.id}
                  onClick={(e) => {
                    e.stopPropagation();
                    handleDelete(t.id);
                  }}
                >
                  {busyId === t.id ? '...' : '삭제'}
                </button>
              </div>
            ),
          },
        ]}
      />
    </PanelLayout>
  );
}
