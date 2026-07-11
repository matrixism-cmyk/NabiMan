import React, { useState } from 'react';
import { useT } from '../../i18n';
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
  const { t } = useT();
  const { data, loading, error, refetch } = useMecApi<Tenant>(
    `/api/mec/v1/tenants/${encodeURIComponent(tenantId)}`,
    30_000,
  );
  const [tab, setTab] = useState<TabKey>('overview');

  if (loading && !data) return <div className="panel-loading">{t('mec.state.loading')}</div>;
  if (error) return <div className="panel-error">{error}</div>;
  if (!data) {
    return (
      <div className="panel">
        <div className="panel-empty">{t('mec.tenant.notFound')}</div>
        <button className="btn btn-secondary" onClick={onClose}>{t('mec.tenant.backToList')}</button>
      </div>
    );
  }

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>
          {t('mec.tenant.detailHeading', { name: data.display_name })} <code style={{ fontSize: '14px' }}>({data.id})</code>
        </h2>
        <div>
          <button className="btn btn-secondary" onClick={refetch} style={{ marginRight: '8px' }}>
            {t('mec.action.refresh')}
          </button>
          <button className="btn btn-secondary" onClick={onClose}>
            {t('mec.tenant.backToList')}
          </button>
        </div>
      </div>

      <div className="tab-bar" style={{ display: 'flex', gap: '4px', marginBottom: '16px', borderBottom: '1px solid #e5e7eb' }}>
        {TABS.map((tabItem) => (
          <button
            key={tabItem.key}
            className={tab === tabItem.key ? 'tab-btn active' : 'tab-btn'}
            onClick={() => setTab(tabItem.key)}
            style={{
              background: 'none',
              border: 'none',
              padding: '8px 16px',
              cursor: 'pointer',
              borderBottom: tab === tabItem.key ? '2px solid #3b82f6' : '2px solid transparent',
              fontWeight: tab === tabItem.key ? 600 : 400,
              color: tab === tabItem.key ? '#111827' : '#6b7280',
            }}
          >
            {tabItem.label}
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
