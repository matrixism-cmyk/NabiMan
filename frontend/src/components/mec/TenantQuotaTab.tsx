import React from 'react';
import { useMecApi } from '../../hooks/mec/useMecApi';
import { BarChart, ErrorBanner, Section } from './common';

interface Props {
  tenantId: string;
}

interface QuotaUsage {
  cpu_requests_used: string;
  cpu_requests_hard: string;
  cpu_limits_used: string;
  cpu_limits_hard: string;
  memory_requests_used: string;
  memory_requests_hard: string;
  memory_limits_used: string;
  memory_limits_hard: string;
  gpu_used: number;
  gpu_hard: number;
  pods_used: number;
  pods_hard: number;
  lb_services_used: number;
  lb_services_hard: number;
  storage_used: string;
  storage_hard: string;
}

function parseNumeric(s: string): number {
  const m = /^([\d.]+)/.exec(s);
  return m ? parseFloat(m[1]) : 0;
}

export default function TenantQuotaTab({ tenantId }: Props) {
  const { data, loading, error, refetch } = useMecApi<QuotaUsage>(
    `/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/quota-usage`,
    15_000,
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
          새로고침
        </button>
      </div>
      {loading && !data && (
        <div style={{ color: 'var(--text-secondary)' }}>로딩 중...</div>
      )}
      <ErrorBanner error={error || undefined} />

      {data && (
        <Section title="ResourceQuota 사용률">
          <BarChart
            data={[
              {
                label: 'CPU requests',
                value: parseNumeric(data.cpu_requests_used),
                max: parseNumeric(data.cpu_requests_hard),
                helper: `${data.cpu_requests_used} / ${data.cpu_requests_hard}`,
              },
              {
                label: 'CPU limits',
                value: parseNumeric(data.cpu_limits_used),
                max: parseNumeric(data.cpu_limits_hard),
                helper: `${data.cpu_limits_used} / ${data.cpu_limits_hard}`,
              },
              {
                label: 'Memory requests',
                value: parseNumeric(data.memory_requests_used),
                max: parseNumeric(data.memory_requests_hard),
                helper: `${data.memory_requests_used} / ${data.memory_requests_hard}`,
              },
              {
                label: 'Memory limits',
                value: parseNumeric(data.memory_limits_used),
                max: parseNumeric(data.memory_limits_hard),
                helper: `${data.memory_limits_used} / ${data.memory_limits_hard}`,
              },
              {
                label: 'GPU slots',
                value: data.gpu_used,
                max: data.gpu_hard,
              },
              {
                label: 'Pods',
                value: data.pods_used,
                max: data.pods_hard,
              },
              {
                label: 'LB Services',
                value: data.lb_services_used,
                max: data.lb_services_hard,
              },
              {
                label: 'Storage',
                value: parseNumeric(data.storage_used),
                max: parseNumeric(data.storage_hard),
                helper: `${data.storage_used} / ${data.storage_hard}`,
              },
            ]}
          />
        </Section>
      )}
    </div>
  );
}
