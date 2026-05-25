import React from 'react';
import { useMecApi } from '../../hooks/mec/useMecApi';
import { GpuOverview } from '../../types/mec';
import {
  BarChart,
  DonutChart,
  ErrorBanner,
  MetricCard,
  MetricGrid,
  PanelLayout,
  Section,
} from './common';

const MODEL_COLORS: Record<string, string> = {
  T4: '#10b981',
  A40: '#8b5cf6',
  A100: '#f59e0b',
  H100: '#ef4444',
  GH200: '#06b6d4',
};

function modelColor(m: string): string {
  for (const k of Object.keys(MODEL_COLORS)) {
    if (m.toUpperCase().includes(k)) return MODEL_COLORS[k];
  }
  return '#3b82f6';
}

export default function MecGpuPanel() {
  const { data, loading, error, refetch } = useMecApi<GpuOverview>(
    '/api/mec/v1/gpu/overview',
    30_000,
  );

  const pct = data && data.total_slots > 0
    ? Math.round((data.allocated_slots / data.total_slots) * 100)
    : 0;

  return (
    <PanelLayout
      title="GPU 자원"
      subtitle={data ? `총 ${data.total_slots} slots · ${pct}% 할당` : '로딩 중...'}
      actions={
        <button className="btn btn-secondary" onClick={refetch}>
          새로고침
        </button>
      }
    >
      <ErrorBanner error={error || undefined} />
      {loading && !data && <div style={{ color: '#6b7280' }}>로딩 중...</div>}

      {data && (
        <>
          <MetricGrid>
            <MetricCard
              label="전체 Slots"
              value={data.total_slots}
              helper="GPU 공유 전략 반영"
              tone="info"
            />
            <MetricCard
              label="할당"
              value={data.allocated_slots}
              helper={`${pct}% 사용`}
              tone={pct >= 90 ? 'error' : pct >= 70 ? 'warning' : 'info'}
            />
            <MetricCard
              label="가용"
              value={data.available_slots}
              helper="즉시 할당 가능"
              tone={data.available_slots === 0 ? 'error' : 'success'}
            />
            <MetricCard
              label="GPU 모델 종류"
              value={data.by_model.length}
              helper={data.by_model.map((m) => m.model).join(', ')}
              tone="neutral"
            />
          </MetricGrid>

          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
              gap: '16px',
              marginTop: '20px',
            }}
          >
            <Section title="전체 GPU 사용률">
              <DonutChart
                slices={[
                  { label: '할당', value: data.allocated_slots, color: '#3b82f6' },
                  { label: '가용', value: data.available_slots, color: '#e5e7eb' },
                ]}
                centerLabel={`${pct}%`}
                centerSublabel={`${data.allocated_slots}/${data.total_slots}`}
                legend="side"
              />
            </Section>

            <Section title="모델별 slots 분포">
              <DonutChart
                slices={data.by_model.map((m) => ({
                  label: `${m.model} (${m.allocated_slots}/${m.total_slots})`,
                  value: m.total_slots,
                  color: modelColor(m.model),
                }))}
                centerLabel={data.by_model.length.toString()}
                centerSublabel="모델"
                legend="side"
              />
            </Section>
          </div>

          <Section title="모델별 상세" marginTop="24px">
            <BarChart
              data={data.by_model.map((m) => ({
                label: `${m.model}  (${m.sharing_strategy})`,
                value: m.allocated_slots,
                max: m.total_slots,
                color: modelColor(m.model),
                helper: `노드: ${m.nodes.join(', ') || '-'}`,
              }))}
              showValue
            />
          </Section>
        </>
      )}
    </PanelLayout>
  );
}
