import React, { useEffect, useState } from 'react';
import { apiRequest } from '../../shared/api/useApi';
import { useT } from '../../i18n';
import type { AccessRules, MemberUser } from '../MembershipEditor';
import { formatDateTime } from '../../shared/i18n/format';

const HOURS = Array.from({ length: 25 }, (_, h) => h);

/** datetime-local's value for an instant, in this browser's time. */
function local(iso?: string | null): string {
  if (!iso) return '';
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? '' : new Date(d.getTime() - d.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
}

const privateAddress = (ip: string) => /^(10\.|127\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.|::1$|f[cd])/i.test(ip);

interface Props {
  user: MemberUser;
  busy: boolean;
  update: (body: object) => Promise<boolean>;
}

/** When and from where the account may be used, and how it signs in. */
export default function AccessRulesForm({ user, busy, update }: Props) {
  const saved: AccessRules = { expires_at: null, require_mfa: false, allowed_networks: [], allowed_hours: null, ...user.access };
  const [draft, setDraft] = useState<AccessRules>(saved);
  const [networks, setNetworks] = useState(saved.allowed_networks.join('\n'));
  const { t } = useT();
  const days = t('users.access.days').split(',');
  const [mine, setMine] = useState('');
  // Without it an admin could lock themselves out unawares, so a failed lookup is said.
  const [mineFailed, setMineFailed] = useState(false);
  useEffect(() => {
    apiRequest<string>('/api/users/my-address')
      .then(r => { if (r.success && r.data) setMine(r.data); else setMineFailed(true); })
      .catch(() => setMineFailed(true));
  }, []);

  const next: AccessRules = { ...draft, allowed_networks: networks.split(/[\s,]+/).map(s => s.trim()).filter(Boolean) };
  const changed = JSON.stringify(next) !== JSON.stringify(saved);
  const hours = draft.allowed_hours;
  const setHours = (h: AccessRules['allowed_hours']) => setDraft(d => ({ ...d, allowed_hours: h }));
  const expireIn = (days: number) => setDraft(d => ({ ...d, expires_at: new Date(Date.now() + days * 86400000).toISOString() }));

  return (
    <div className="users-sections">
      <section className="users-card">
        <h4>{t('users.access.period')}</h4>
        <div className="users-row">
          <label>{t('users.access.expiry')}
            <input type="datetime-local" className="filter-input" value={local(draft.expires_at)}
              onChange={e => setDraft(d => ({ ...d, expires_at: e.target.value ? new Date(e.target.value).toISOString() : null }))} />
          </label>
          <div className="btn-group">
            {[30, 90, 365].map(n => <button key={n} type="button" className="btn btn-secondary btn-sm" onClick={() => expireIn(n)}>{n === 365 ? t('users.access.plusYear') : t('users.access.plusDays', { n })}</button>)}
            <button type="button" className="btn btn-secondary btn-sm" onClick={() => setDraft(d => ({ ...d, expires_at: null }))}>{t('users.access.noDeadline')}</button>
          </div>
        </div>
        <p className="text-secondary">{draft.expires_at
          ? t('users.access.until', { when: formatDateTime(draft.expires_at) })
          : t('users.access.forever')}</p>
      </section>

      <section className="users-card">
        <h4>{t('users.access.mfa')}</h4>
        <label className={`users-grant ${draft.require_mfa ? 'is-on' : ''}`}>
          <input type="checkbox" checked={draft.require_mfa} onChange={e => setDraft(d => ({ ...d, require_mfa: e.target.checked }))} />
          <span><strong>{t('users.access.mfaRequired')}</strong>
            <small>{user.totp_enabled ? t('users.access.mfaHas')
              : t('users.access.mfaMissing')}</small></span>
        </label>
      </section>

      <section className="users-card">
        <h4>{t('users.access.networks')}</h4>
        <label>{t('users.access.networksHint')}
          <textarea className="filter-input users-networks" rows={4} value={networks} spellCheck={false}
            onChange={e => setNetworks(e.target.value)} placeholder={'203.0.113.7\n198.51.100.0/24'} />
        </label>
        {mineFailed && <div role="alert" className="message">{t('access.myAddressFailed')}</div>}
        {mine && (
          <p className="text-secondary">
            {t('users.access.myAddress')}<code>{mine}</code>{' '}
            <button type="button" className="btn btn-secondary btn-sm"
              onClick={() => setNetworks(n => (n.trim() ? `${n.trim()}\n${mine}` : mine))}>{t('users.access.addMine')}</button>
            {privateAddress(mine) && <span className="users-warn">{t('users.access.privateWarn')}</span>}
          </p>
        )}
      </section>

      <section className="users-card">
        <h4>{t('users.access.hours')}</h4>
        <label className={`users-grant ${hours ? 'is-on' : ''}`}>
          <input type="checkbox" checked={Boolean(hours)}
            onChange={e => setHours(e.target.checked ? { start: 9, end: 18, days: [0, 1, 2, 3, 4] } : null)} />
          <span><strong>{t('users.access.hoursOnly')}</strong><small>{t('users.access.hoursHint')}</small></span>
        </label>
        {hours && (
          <div className="users-hours">
            <label>{t('users.access.start')}
              <select className="select-input" value={hours.start} onChange={e => setHours({ ...hours, start: Number(e.target.value) })}>
                {HOURS.slice(0, 24).map(h => <option key={h} value={h}>{String(h).padStart(2, '0')}:00</option>)}
              </select>
            </label>
            <label>{t('users.access.end')}
              <select className="select-input" value={hours.end} onChange={e => setHours({ ...hours, end: Number(e.target.value) })}>
                {HOURS.slice(1).map(h => <option key={h} value={h}>{String(h).padStart(2, '0')}:00</option>)}
              </select>
            </label>
            <div className="users-days" role="group" aria-label={t('users.access.dayGroup')}>
              {days.map((d, i) => (
                <button key={d} type="button" aria-pressed={hours.days.includes(i)} className={hours.days.includes(i) ? 'is-on' : ''}
                  onClick={() => setHours({ ...hours, days: hours.days.includes(i) ? hours.days.filter(x => x !== i) : [...hours.days, i].sort() })}>{d}</button>
              ))}
            </div>
            <small className="text-secondary">{hours.end <= hours.start ? t('users.access.overnight') : ''}{t('users.access.everyDay')}</small>
          </div>
        )}
      </section>

      <div className="btn-group users-save">
        <button type="button" className="btn btn-primary btn-sm" disabled={busy || !changed}
          onClick={() => update({ access: next })}>{t('users.access.save')}</button>
        <button type="button" className="btn btn-secondary btn-sm" disabled={!changed}
          onClick={() => { setDraft(saved); setNetworks(saved.allowed_networks.join('\n')); }}>{t('users.revert')}</button>
      </div>
    </div>
  );
}
