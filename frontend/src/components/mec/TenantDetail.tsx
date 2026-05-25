import React, { useState } from 'react';
import { useMecApi } from '../../hooks/mec/useMecApi';
import { Tenant } from '../../types/mec';
import TenantOverviewTab from './TenantOverviewTab';
import TenantResourcesTab from './TenantResourcesTab';
import TenantQuotaTab from './TenantQuotaTab';
import TenantAuditTab from './TenantAuditTab';
import TenantStarterKitTab from './TenantStarterKitTab';
import TenantMembersTab from './TenantMembersTab';

type TabKey = 'overview' | 'resources' | 'quota' | 'members' | 'starter' | 'audit';

const TABS: { key: TabKey; label: string }[] = [
  { key: 'overview', label: 'Overview' },
  { key: 'resources', label: 'Resources' },
  { key: 'quota', label: 'Quota' },
  { key: 'members', label: 'Members' },
  { key: 'starter', label: 'Starter Kit' },
  { key: 'audit', label: 'Audit' },
];

interface Props {
  tenantId: string;
  onClose: () => void;
}

export default function TenantDetail({ tenantId, onClose }: Props) {
  const { data, loading, error, refetch } = useMecApi<Tenant>(
    `/api/mec/v1/tenants/${encodeURIComponent(tenantId)}`,
    30_000,
  );
  const [tab, setTab] = useState<TabKey>('overview');

  if (loading && !data) return <div className="panel-loading">로딩 중...</div>;
  if (error) return <div className="panel-error">{error}</div>;
  if (!data) {
    return (
      <div className="panel">
        <div className="panel-empty">테넌트를 찾을 수 없습니다.</div>
        <button className="btn btn-secondary" onClick={onClose}>목록으로</button>
      </div>
    );
  }

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>
          테넌트 — {data.display_name} <code style={{ fontSize: '14px' }}>({data.id})</code>
        </h2>
        <div>
          <button className="btn btn-secondary" onClick={refetch} style={{ marginRight: '8px' }}>
            새로고침
          </button>
          <button className="btn btn-secondary" onClick={onClose}>
            목록으로
          </button>
        </div>
      </div>

      <div className="tab-bar" style={{ display: 'flex', gap: '4px', marginBottom: '16px', borderBottom: '1px solid #e5e7eb' }}>
        {TABS.map((t) => (
          <button
            key={t.key}
            className={tab === t.key ? 'tab-btn active' : 'tab-btn'}
            onClick={() => setTab(t.key)}
            style={{
              background: 'none',
              border: 'none',
              padding: '8px 16px',
              cursor: 'pointer',
              borderBottom: tab === t.key ? '2px solid #3b82f6' : '2px solid transparent',
              fontWeight: tab === t.key ? 600 : 400,
              color: tab === t.key ? '#111827' : '#6b7280',
            }}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === 'overview' && <TenantOverviewTab tenant={data} />}
      {tab === 'resources' && <TenantResourcesTab tenantId={data.id} />}
      {tab === 'quota' && <TenantQuotaTab tenantId={data.id} />}
      {tab === 'members' && <TenantMembersTab tenantId={data.id} />}
      {tab === 'starter' && <TenantStarterKitTab tenantId={data.id} onChanged={refetch} />}
      {tab === 'audit' && <TenantAuditTab tenantId={data.id} />}
    </div>
  );
}
