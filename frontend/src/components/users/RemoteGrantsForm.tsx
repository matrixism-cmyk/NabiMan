import React, { useState } from 'react';
import type { MemberUser } from '../MembershipEditor';
import { DEFAULT_GRANTS, MemberGrants } from '../remote/member';
import { useT } from '../../i18n';

type Flag = { [K in keyof MemberGrants]: MemberGrants[K] extends boolean ? K : never }[keyof MemberGrants];
type Limit = { [K in keyof MemberGrants]: MemberGrants[K] extends number ? K : never }[keyof MemberGrants];

// Each item's name and hint are `users.grants.<key>` and `users.grants.<key>.hint`.
const SHARING: Flag[] = ['share_sessions', 'share_files', 'share_require_password', 'share_view_only'];
const FEATURES: Flag[] = ['dictation', 'register_servers', 'assigned_file_write', 'copy_between_servers', 'keep_sessions'];
const LIMITS: { key: Limit; unit: string; max: number }[] = [
  { key: 'share_max_days', unit: 'users.grants.unit.days', max: 90 },
  { key: 'max_sessions', unit: 'users.grants.unit.count', max: 100 },
  { key: 'max_idle_minutes', unit: 'users.grants.unit.minutes', max: 43200 },
  { key: 'max_servers', unit: 'users.grants.unit.servers', max: 1000 },
  { key: 'max_upload_mb', unit: '', max: 1048576 },
];

interface Props {
  user: MemberUser;
  busy: boolean;
  update: (body: object) => Promise<boolean>;
}

/** What a remote-only member may do beyond working on their servers. Limits of 0 mean none. */
export default function RemoteGrantsForm({ user, busy, update }: Props) {
  const saved: MemberGrants = { ...DEFAULT_GRANTS, ...user.remote_permissions };
  const [draft, setDraft] = useState<MemberGrants>(saved);
  const changed = JSON.stringify(draft) !== JSON.stringify(saved);
  const { t } = useT();
  const set = <K extends keyof MemberGrants>(key: K, value: MemberGrants[K]) => setDraft(d => ({ ...d, [key]: value }));

  const flags = (title: string, items: Flag[], note?: string) => (
    <section className="users-card">
      <h4>{title}</h4>
      {note && <p className="text-secondary">{note}</p>}
      <div className="users-grants">
        {items.map(key => {
          const off = (key === 'share_require_password' || key === 'share_view_only')
            && !draft.share_sessions && !draft.share_files;
          return (
            <label key={key} className={`users-grant ${draft[key] ? 'is-on' : ''} ${off ? 'is-dim' : ''}`}>
              <input type="checkbox" checked={draft[key]} onChange={e => set(key, e.target.checked)} />
              <span><strong>{t(`users.grants.${key}`)}</strong><small>{t(`users.grants.${key}.hint`)}</small></span>
            </label>
          );
        })}
      </div>
    </section>
  );

  return (
    <div className="users-sections">
      {flags(t('users.grants.sharing'), SHARING, t('users.grants.sharingNote'))}
      {flags(t('users.grants.features'), FEATURES)}
      <section className="users-card">
        <h4>{t('users.grants.limits')} <span className="text-secondary">{t('users.grants.limitsZero')}</span></h4>
        <div className="users-limits">
          {LIMITS.map(item => (
            <label key={item.key} className="users-limit">
              <span>{t(`users.grants.${item.key}`)}</span>
              <span className="users-limit-input">
                <input type="number" min={0} max={item.max} className="filter-input" value={draft[item.key]}
                  onChange={e => set(item.key, Math.max(0, Math.min(item.max, Math.floor(Number(e.target.value) || 0))))} />
                <em>{item.unit ? t(item.unit) : 'MB'}</em>
              </span>
              <small>{t(`users.grants.${item.key}.hint`)}</small>
            </label>
          ))}
        </div>
      </section>
      <div className="btn-group users-save">
        <button type="button" className="btn btn-primary btn-sm" disabled={busy || !changed}
          onClick={() => update({ remote_permissions: draft })}>{t('users.grants.save')}</button>
        <button type="button" className="btn btn-secondary btn-sm" disabled={!changed} onClick={() => setDraft(saved)}>{t('users.revert')}</button>
        <button type="button" className="btn btn-secondary btn-sm" onClick={() => setDraft(DEFAULT_GRANTS)}>{t('users.grants.defaults')}</button>
      </div>
      <p className="text-secondary">{t('users.grants.note')}</p>
    </div>
  );
}
