import React, { useMemo, useState } from 'react';
import { useT } from '../../i18n';
import { useJobStream, JobStreamEvent, JobSnapshotStatus } from '../../hooks/mec/useJobStream';

interface StepState {
  name: string;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'skipped';
  duration_ms?: number | null;
  message?: string | null;
  reason?: string;
}

interface Props {
  jobId: string;
}

export default function JobProgress({ jobId }: Props) {
  const { t } = useT();
  const [steps, setSteps] = useState<Record<string, StepState>>({});
  const [order, setOrder] = useState<string[]>([]);
  const [finalStatus, setFinalStatus] = useState<JobSnapshotStatus['state']>(
    'running',
  );
  const [terminalError, setTerminalError] = useState<string | null>(null);

  const { isOpen, error } = useJobStream({
    jobId,
    onEvent: (ev: JobStreamEvent) => {
      switch (ev.event) {
        case 'snapshot':
          setFinalStatus(ev.status.state);
          setTerminalError(ev.status.reason || ev.status.error || ev.error || null);
          setSteps(Object.fromEntries((ev.steps || []).map((step) => [step.name, step])));
          setOrder((ev.steps || []).map((step) => step.name));
          break;
        case 'step_started':
          setOrder((prev) =>
            prev.includes(ev.step) ? prev : [...prev, ev.step],
          );
          setSteps((prev) => ({
            ...prev,
            [ev.step]: { name: ev.step, status: 'running' },
          }));
          break;
        case 'step_completed':
          setSteps((prev) => ({
            ...prev,
            [ev.step]: {
              name: ev.step,
              status: 'completed',
              duration_ms: ev.duration_ms,
              message: ev.message,
            },
          }));
          break;
        case 'step_failed':
          setSteps((prev) => ({
            ...prev,
            [ev.step]: {
              name: ev.step,
              status: 'failed',
              reason: ev.reason,
            },
          }));
          break;
        case 'completed':
          setFinalStatus('completed');
          break;
        case 'failed':
          setFinalStatus('failed');
          setTerminalError(ev.error);
          break;
        default:
          break;
      }
    },
  });

  const progress = useMemo(() => {
    const total = order.length || 1;
    const done = Object.values(steps).filter(
      (s) => s.status === 'completed' || s.status === 'skipped',
    ).length;
    return Math.min(100, Math.round((done / total) * 100));
  }, [order, steps]);

  return (
    <div style={{ padding: '8px 0' }}>
      <div
        style={{
          background: 'var(--track)',
          height: '12px',
          borderRadius: '3px',
          overflow: 'hidden',
          marginBottom: '12px',
        }}
      >
        <div
          style={{
            background: barColor(finalStatus),
            height: '100%',
            width: `${progress}%`,
            transition: 'width 0.3s',
          }}
        />
      </div>

      <div style={{ fontSize: '13px', marginBottom: '8px' }}>
        {t('mec.jobs.statusLabel')}{' '}
        <strong style={{ color: barColor(finalStatus) }}>
          {finalStatus === 'running'
            ? (isOpen ? t('mec.jobs.stateRunning') : t('mec.jobs.stateConnecting'))
            : finalStatus === 'unknown_result' ? t('mec.jobs.stateUnknownResult') : finalStatus}
        </strong>
        {error && <span style={{ color: 'var(--danger)' }}> — {error}</span>}
        {terminalError && (
          <div
            style={{
              color: 'var(--danger)',
              marginTop: '4px',
              fontSize: '12px',
            }}
          >
            {t('mec.jobs.errorLabel')} {terminalError}
          </div>
        )}
      </div>

      {finalStatus === 'unknown_result' && (
        <div role="alert" style={{ color: 'var(--warning-strong)', background: 'var(--warning-soft)', padding: '10px', marginBottom: '12px' }}>
          {t('mec.jobs.verifyBeforeRetry')}
        </div>
      )}

      <ul
        style={{
          listStyle: 'none',
          padding: 0,
          margin: 0,
          fontFamily: 'monospace',
          fontSize: '13px',
        }}
      >
        {order.map((name) => {
          const s = steps[name];
          const icon =
            s.status === 'completed'
              ? '✓'
              : s.status === 'running'
              ? (finalStatus === 'unknown_result' ? '?' : '⟳')
              : s.status === 'failed'
              ? '✗'
              : '·';
          return (
            <li
              key={name}
              style={{ padding: '3px 0', color: finalStatus === 'unknown_result' && s.status === 'running' ? 'var(--warning-strong)' : stepColor(s.status) }}
            >
              <span style={{ display: 'inline-block', width: '18px' }}>
                {icon}
              </span>
              {s.name}
              {s.duration_ms != null && (
                <span style={{ color: 'var(--text-muted)', marginLeft: '8px' }}>
                  ({s.duration_ms}ms)
                </span>
              )}
              {s.reason && (
                <span style={{ color: 'var(--danger)', marginLeft: '8px' }}>
                  — {s.reason}
                </span>
              )}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

function barColor(state: string): string {
  if (state === 'completed') return 'var(--success)';
  if (state === 'failed') return 'var(--danger)';
  if (state === 'unknown_result') return 'var(--warning-strong)';
  return 'var(--info)';
}

function stepColor(state: string): string {
  if (state === 'completed') return 'var(--success)';
  if (state === 'failed') return 'var(--danger)';
  if (state === 'running') return 'var(--info)';
  return 'var(--text-secondary)';
}
