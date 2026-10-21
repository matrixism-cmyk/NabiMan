import React, { useState } from 'react';
import { useT } from '../../i18n';
import { useMecList } from '../../hooks/mec/useMecApi';
import JobProgress from './JobProgress';
import RecoveryInspection from './RecoveryInspection';
import { useApi } from '../../shared/api/useApi';
import { MemberUser } from '../MembershipEditor';
import {
  ErrorBanner,
  PanelLayout,
  SortableTable,
  StatusBadge,
  Toolbar,
} from './common';

interface JobSummary {
  id: string;
  kind: string;
  status: { state: string; error?: string; reason?: string };
  started_at: string;
  completed_at?: string | null;
  progress_percent: number;
  steps: Array<{
    name: string;
    status: string;
    duration_ms?: number | null;
    message?: string | null;
  }>;
}

function ProgressBar({ value, tone }: { value: number; tone: string }) {
  const color =
    tone === 'completed' ? '#10b981' :
    tone === 'failed' ? '#ef4444' :
    tone === 'unknown_result' ? '#d97706' :
    tone === 'cancelled' ? '#9ca3af' :
    '#3b82f6';
  return (
    <div style={{ minWidth: '100px' }}>
      <div
        style={{
          background: '#f3f4f6',
          height: '8px',
          borderRadius: '4px',
          overflow: 'hidden',
        }}
      >
        <div
          style={{
            background: color,
            height: '100%',
            width: `${value}%`,
            transition: 'width 0.3s',
          }}
        />
      </div>
      <span style={{ fontSize: '11px', color: '#6b7280' }}>{value}%</span>
    </div>
  );
}

export default function MecJobsPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useMecList<JobSummary>(
    '/api/mec/v1/jobs',
    5_000,
  );
  const { data: me } = useApi<MemberUser>('/api/users/me');
  const [recoveryId, setRecoveryId] = useState<string | null>(null);
  const [streamingId, setStreamingId] = useState<string | null>(null);
  const [filter, setFilter] = useState('');

  const jobs = data || [];
  const running = jobs.filter(
    (j) => j.status.state === 'running' || j.status.state === 'pending',
  ).length;

  return (
    <PanelLayout
      title={t('mec.jobs.title')}
      subtitle={t('mec.jobs.subtitle', { total: jobs.length, running })}
      actions={
        <button className="btn btn-secondary" onClick={refetch}>
          {t('mec.action.refresh')}
        </button>
      }
    >
      <ErrorBanner error={error || undefined} />

      {streamingId && (
        <div
          style={{
            background: 'var(--surface-alt)',
            color: 'var(--text)',
            border: '1px solid var(--border)',
            borderRadius: '8px',
            padding: '14px',
            marginBottom: '16px',
          }}
        >
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              marginBottom: '8px',
            }}
          >
            <h3 style={{ margin: 0, fontSize: '14px' }}>
              {t('mec.jobs.liveProgress')}: <code>{streamingId}</code>
            </h3>
            <button
              className="btn btn-secondary btn-small"
              onClick={() => setStreamingId(null)}
            >
              {t('mec.jobs.close')}
            </button>
          </div>
          <JobProgress key={streamingId} jobId={streamingId} />
        </div>
      )}

      {recoveryId && <RecoveryInspection key={recoveryId} jobId={recoveryId} />}

      <Toolbar>
        <input
          placeholder={t('mec.jobs.filterPlaceholder')}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          style={{ padding: '6px 10px', minWidth: '240px' }}
        />
      </Toolbar>

      {loading && !data && <div style={{ color: '#6b7280' }}>{t('mec.state.loading')}</div>}

      <SortableTable<JobSummary>
        data={jobs}
        filter={filter}
        defaultSortKey="started_at"
        defaultSortDir="desc"
        rowKey={(j) => j.id}
        emptyMessage={t('mec.jobs.empty')}
        columns={[
          {
            key: 'id',
            header: t('mec.jobs.colId'),
            accessor: (j) => j.id,
            render: (j) => <code style={{ display: 'block', maxWidth: '180px', whiteSpace: 'normal', overflowWrap: 'anywhere' }}>{j.id}</code>,
            sortable: true,
          },
          {
            key: 'kind',
            header: t('mec.jobs.colKind'),
            accessor: (j) => j.kind,
            render: (j) => <code style={{ fontSize: '12px' }}>{j.kind}</code>,
            sortable: true,
          },
          {
            key: 'status',
            header: t('mec.jobs.colStatus'),
            accessor: (j) => j.status.state,
            render: (j) => (
              <div>
                <StatusBadge tone={j.status.state}>
                  {j.status.state === 'unknown_result' ? t('mec.jobs.stateUnknownResult') : j.status.state}
                </StatusBadge>
                {(j.status.error || j.status.reason) && (
                  <div
                    style={{
                      color: '#ef4444',
                      fontSize: '11px',
                      marginTop: '2px',
                      maxWidth: '220px',
                      overflowWrap: 'anywhere',
                    }}
                  >
                    {j.status.reason || j.status.error}
                  </div>
                )}
              </div>
            ),
            width: '180px',
            wrap: true,
            sortable: true,
          },
          {
            key: 'progress',
            header: t('mec.jobs.colProgress'),
            accessor: (j) => j.progress_percent,
            render: (j) => (
              <ProgressBar value={j.progress_percent} tone={j.status.state} />
            ),
            width: '140px',
            sortable: true,
          },
          {
            key: 'steps',
            header: t('mec.jobs.colSteps'),
            accessor: (j) =>
              j.steps.filter((s) => s.status === 'completed').length,
            render: (j) => (
              <span style={{ fontSize: '12px', color: '#6b7280' }}>
                {j.steps.filter((s) => s.status === 'completed').length}/
                {j.steps.length}
              </span>
            ),
            width: '80px',
            align: 'right',
            sortable: true,
          },
          {
            key: 'started_at',
            header: t('mec.jobs.colStarted'),
            accessor: (j) => j.started_at,
            render: (j) => new Date(j.started_at).toLocaleString(),
            width: '170px',
            sortable: true,
          },
          {
            key: 'completed_at',
            header: t('mec.jobs.colCompleted'),
            accessor: (j) => j.completed_at || '',
            render: (j) =>
              j.completed_at ? (
                new Date(j.completed_at).toLocaleString()
              ) : (
                <span style={{ color: '#d1d5db' }}>-</span>
              ),
            width: '170px',
            sortable: true,
          },
          {
            key: 'actions',
            header: '',
            accessor: () => '',
            sortable: false,
            width: '100px',
            render: (j) => (
              <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap' }}>
              <button
                className={
                  j.status.state === 'running' || j.status.state === 'pending'
                    ? 'btn btn-primary btn-small'
                    : 'btn btn-secondary btn-small'
                }
                onClick={() => setStreamingId(j.id)}
              >
                {j.status.state === 'running' || j.status.state === 'pending'
                  ? t('mec.jobs.actionLive')
                  : t('mec.jobs.actionHistory')}
              </button>
              {me?.role === 'admin' && ['failed', 'unknown_result'].includes(j.status.state) && ['tenant_create', 'tenant_delete'].includes(j.kind) && <button className="btn btn-secondary btn-small" onClick={() => setRecoveryId(j.id)}>{t('mec.recovery.inspect')}</button>}
              </div>
            ),
          },
        ]}
      />
    </PanelLayout>
  );
}
