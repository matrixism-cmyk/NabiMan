import React, { useEffect, useState } from 'react';
import { apiRequest } from '../../../shared/api/useApi';
import { useT } from '../../../shared/i18n';
import { startVisibleInterval } from '../../../shared/ui/useVisibleInterval';
import { formatTime, formatNumber } from '../../../shared/i18n/format';

interface Attacker { ip: string; count: number; last: number; users: string[]; internal: boolean; banned: boolean }
interface Login { ip: string; user: string; at: number; method: string }
export interface AttackReport {
  readable: boolean; source: string; window_min: number; failed: number; kinds: Record<string, number>;
  attackers: Attacker[]; sources: number; per_minute: number[]; accepted: Login[]; banned: string[]; checked_at: number; error?: string;
}

const WINDOWS = [15, 60, 360, 1440];
const REFRESH_MS = 30000;
const KINDS = ['failed_password', 'invalid_user', 'preauth_closed'];

/** Who is trying to log in to a machine over SSH, read from its log every half minute. */
export default function AttackPanel({ id, name, sub, onClose }: { id: string | null; name: string; sub?: string; onClose: () => void }) {
  const { t } = useT();
  const [windowMin, setWindowMin] = useState(60);
  const [report, setReport] = useState<AttackReport | null>(null);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    setReport(null); setError('');
    if (!id) return undefined;
    let gone = false;
    const load = () => {
      setLoading(true);
      apiRequest(`/api/remote-servers/${encodeURIComponent(id)}/attacks?window=${windowMin}`)
        .then((r: any) => {
          if (gone) return;
          if (r?.success && r.data) { setReport(r.data); setError(r.data.error || ''); } else setError(r?.message || t('attack.failed'));
        })
        .catch(() => { if (!gone) setError(t('attack.failed')); })
        .finally(() => { if (!gone) setLoading(false); });
    };
    load();
    const stopPolling = startVisibleInterval(load, REFRESH_MS);
    return () => { gone = true; stopPolling(); };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, windowMin]);

  const clock = (at: number) => (at ? formatTime(at * 1000, { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false }) : '–');
  const label = (m: number) => (m < 60 ? t('attack.minutes', { n: m }) : t('attack.hours', { n: m / 60 }));
  const peak = Math.max(1, ...(report?.per_minute || []));
  const outside = report ? report.attackers.filter((a) => !a.internal) : [];

  return (
    <aside className="session-graph-panel attack-panel" aria-label={t('attack.title', { name })}>
      <div className="session-graph-panel-head">
        <strong>🛡 {name}</strong>
        <button type="button" className="btn btn-secondary btn-sm" onClick={onClose} aria-label={t('common.close')}>✕</button>
      </div>
      {sub && <code className="session-label">{sub}</code>}
      <div className="attack-windows" role="group" aria-label={t('attack.window')}>
        {WINDOWS.map((m) => (
          <button key={m} type="button" className={`btn btn-sm ${m === windowMin ? 'btn-primary' : 'btn-secondary'}`} aria-pressed={m === windowMin} onClick={() => setWindowMin(m)}>{label(m)}</button>
        ))}
        <span className="attack-live" aria-live="polite">{loading ? t('attack.reading') : report ? t('attack.asOf', { time: clock(report.checked_at) }) : ''}</span>
      </div>
      {!id && <p className="attack-note">{t('attack.notSaved')}</p>}
      {error && <p className="attack-note is-bad" role="alert">{error}</p>}
      {report && !report.error && !report.readable && <p className="attack-note is-bad">{t(report.source === 'none' ? 'attack.noLog' : 'attack.unreadable')}</p>}
      {report && report.readable && (
        <>
          <div className={`attack-total${outside.length ? ' is-hot' : ''}`}>
            <b>{formatNumber(report.failed)}</b>
            <span>{t('attack.total', { sources: report.sources })}</span>
          </div>
          <div className="attack-kinds">
            {KINDS.filter((k) => report.kinds[k]).map((k) => <span key={k} className="attack-chip">{t(`attack.kind.${k}`)} <b>{report.kinds[k]}</b></span>)}
            {report.banned.length > 0 && <span className="attack-chip is-banned">{t('attack.banned')} <b>{report.banned.length}</b></span>}
          </div>
          {report.per_minute.length > 0 && (
            <div className="attack-spark" role="img" aria-label={t('attack.trend')}>
              {report.per_minute.map((n, i) => <i key={i} style={{ height: `${Math.max(n ? 8 : 2, (n / peak) * 100)}%` }} className={n ? 'is-on' : ''} title={String(n)} />)}
            </div>
          )}
          {report.failed === 0 && <p className="attack-note is-good">{t('attack.none')}</p>}
          {report.attackers.length > 0 && (
            <table className="attack-table">
              <thead><tr><th>{t('attack.col.ip')}</th><th>{t('attack.col.count')}</th><th>{t('attack.col.last')}</th><th>{t('attack.col.users')}</th></tr></thead>
              <tbody>
                {report.attackers.map((a) => (
                  <tr key={a.ip} className={a.internal ? 'is-internal' : ''}>
                    <td>
                      <code>{a.ip}</code>
                      {a.internal && <span className="attack-tag">{t('attack.internal')}</span>}
                      {a.banned && <span className="attack-tag is-banned">{t('attack.isBanned')}</span>}
                    </td>
                    <td className="attack-num">{a.count}</td>
                    <td>{clock(a.last)}</td>
                    <td className="attack-users">{a.users.join(', ')}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {report.sources > report.attackers.length && <p className="attack-note">{t('attack.more', { n: report.sources - report.attackers.length })}</p>}
          {report.accepted.length > 0 && (
            <>
              <h4 className="attack-sub">{t('attack.accepted')}</h4>
              <ul className="attack-accepted">
                {report.accepted.slice(0, 12).map((l, i) => (
                  <li key={`${l.at}-${i}`}><time>{clock(l.at)}</time> <b>{l.user}</b> ← <code>{l.ip}</code> <span>{l.method}</span></li>
                ))}
              </ul>
            </>
          )}
          <p className="attack-source">{t('attack.source', { source: report.source })}</p>
        </>
      )}
    </aside>
  );
}
