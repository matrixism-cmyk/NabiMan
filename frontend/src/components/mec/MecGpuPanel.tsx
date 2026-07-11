import React from 'react';
import { useT } from '../../i18n';
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
  const { t } = useT();
  const { data, loading, error, refetch } = useMecApi<GpuOverview>(
    '/api/mec/v1/gpu/overview',
    30_000,
  );

  const pct = data && data.total_slots > 0
    ? Math.round((data.allocated_slots / data.total_slots) * 100)
    : 0;

  return (
    <PanelLayout
      title={t('mec.gpu.title')}
      subtitle={data ? t('mec.gpu.subtitle', { total: data.total_slots, pct }) : t('mec.state.loading')}
      actions={
        <button className="btn btn-secondary" onClick={refetch}>
          {t('mec.action.refresh')}
        </button>
      }
    >
      <ErrorBanner error={error || undefined} />
      {loading && !data && <div style={{ color: '#6b7280' }}>{t('mec.state.loading')}</div>}

      {data && (
        <>
          <MetricGrid>
            <MetricCard
              label={t('mec.gpu.totalSlots')}
              value={data.total_slots}
              helper={t('mec.gpu.totalSlotsHelper')}
              tone="info"
            />
            <MetricCard
              label={t('mec.gpu.allocated')}
              value={data.allocated_slots}
              helper={t('mec.gpu.allocatedHelper', { pct })}
              tone={pct >= 90 ? 'error' : pct >= 70 ? 'warning' : 'info'}
            />
            <MetricCard
              label={t('mec.gpu.available')}
              value={data.available_slots}
              helper={t('mec.gpu.availableHelper')}
              tone={data.available_slots === 0 ? 'error' : 'success'}
            />
            <MetricCard
              label={t('mec.gpu.modelKinds')}
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
            <Section title={t('mec.gpu.overallUsage')}>
              <DonutChart
                slices={[
                  { label: t('mec.gpu.sliceAllocated'), value: data.allocated_slots, color: '#3b82f6' },
                  { label: t('mec.gpu.sliceAvailable'), value: data.available_slots, color: '#e5e7eb' },
                ]}
                centerLabel={`${pct}%`}
                centerSublabel={`${data.allocated_slots}/${data.total_slots}`}
                legend="side"
              />
            </Section>

            <Section title={t('mec.gpu.modelDistribution')}>
              <DonutChart
                slices={data.by_model.map((m) => ({
                  label: `${m.model} (${m.allocated_slots}/${m.total_slots})`,
                  value: m.total_slots,
                  color: modelColor(m.model),
                }))}
                centerLabel={data.by_model.length.toString()}
                centerSublabel={t('mec.gpu.modelCenter')}
                legend="side"
              />
            </Section>
          </div>

          <Section title={t('mec.gpu.modelDetail')} marginTop="24px">
            <BarChart
              data={data.by_model.map((m) => ({
                label: `${m.model}  (${m.sharing_strategy})`,
                value: m.allocated_slots,
                max: m.total_slots,
                color: modelColor(m.model),
                helper: t('mec.gpu.nodesHelper', { nodes: m.nodes.join(', ') || '-' }),
              }))}
              showValue
            />
          </Section>
        </>
      )}
    </PanelLayout>
  );
}
