import React, { useMemo, useState } from 'react';
import { useT } from '../../i18n';
import { useJobStream, JobStreamEvent } from '../../hooks/mec/useJobStream';

interface StepState {
  name: string;
  status: 'pending' | 'running' | 'completed' | 'failed';
  duration_ms?: number;
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
  const [finalStatus, setFinalStatus] = useState<'running' | 'completed' | 'failed'>(
    'running',
  );
  const [terminalError, setTerminalError] = useState<string | null>(null);

  const { isOpen, error } = useJobStream({
    jobId,
    onEvent: (ev: JobStreamEvent) => {
      switch (ev.event) {
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
      (s) => s.status === 'completed' || s.status === 'failed',
    ).length;
    return Math.min(100, Math.round((done / total) * 100));
  }, [order, steps]);

  return (
    <div style={{ padding: '8px 0' }}>
      <div
        style={{
          background: '#e5e7eb',
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
            : finalStatus}
        </strong>
        {error && <span style={{ color: '#ef4444' }}> — {error}</span>}
        {terminalError && (
          <div
            style={{
              color: '#ef4444',
              marginTop: '4px',
              fontSize: '12px',
            }}
          >
            {t('mec.jobs.errorLabel')} {terminalError}
          </div>
        )}
      </div>

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
              ? '⟳'
              : s.status === 'failed'
              ? '✗'
              : '·';
          return (
            <li
              key={name}
              style={{ padding: '3px 0', color: stepColor(s.status) }}
            >
              <span style={{ display: 'inline-block', width: '18px' }}>
                {icon}
              </span>
              {s.name}
              {s.duration_ms !== undefined && (
                <span style={{ color: '#9ca3af', marginLeft: '8px' }}>
                  ({s.duration_ms}ms)
                </span>
              )}
              {s.reason && (
                <span style={{ color: '#ef4444', marginLeft: '8px' }}>
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
  if (state === 'completed') return '#10b981';
  if (state === 'failed') return '#ef4444';
  return '#3b82f6';
}

function stepColor(state: string): string {
  if (state === 'completed') return '#10b981';
  if (state === 'failed') return '#ef4444';
  if (state === 'running') return '#3b82f6';
  return '#6b7280';
}
