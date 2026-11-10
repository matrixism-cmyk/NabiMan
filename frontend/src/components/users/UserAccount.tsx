import React, { useState } from 'react';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import type { MemberUser } from '../MembershipEditor';
import ExternalIdentityEditor from '../ExternalIdentityEditor';
import { useT } from '../../i18n';
import { ROLE_HINT, ROLE_LABEL } from './userModel';

interface Props {
  user: MemberUser;
  self: boolean;
  busy: boolean;
  update: (body: object) => Promise<boolean>;
}

/** The account itself: its level, a new password, and its link to a Rancher account. */
export default function UserAccount({ user, self, busy, update }: Props) {
  const [role, setRole] = useState(user.role);
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [problem, setProblem] = useState('');
  const [linking, setLinking] = useState(false);
  const { t } = useT();
  const roles = ['remote', 'viewer', 'admin', ...(user.role === 'operator' ? ['operator'] : [])];

  const saveRole = async () => {
    if (role === user.role) return;
    if (role === 'admin' && !(await confirmDialog({ message: t('users.account.grantAdmin', { name: user.username }), danger: true }))) return;
    if (user.role === 'admin' && !(await confirmDialog({ message: t('users.account.revokeAdmin', { name: user.username }), danger: true }))) return;
    update({ role });
  };
  const savePassword = async () => {
    if (password.length < 8) { setProblem(t('users.account.shortPassword')); return; }
    if (password !== confirm) { setProblem(t('users.passwordMismatch')); return; }
    setProblem('');
    if (await update({ password })) { setPassword(''); setConfirm(''); }
  };

  return (
    <div className="users-sections">
      <section className="users-card">
        <h4>{t('users.account.level')}</h4>
        <div className="users-role-list" role="radiogroup" aria-label={t('users.account.levelGroup', { name: user.username })}>
          {roles.map(r => (
            <label key={r} className={`users-role-choice ${role === r ? 'is-on' : ''}`}>
              <input type="radio" name={`role-${user.id}`} value={r} checked={role === r} disabled={self} onChange={() => setRole(r)} />
              <strong>{t(ROLE_LABEL[r])}</strong>
              <small>{t(ROLE_HINT[r])}</small>
            </label>
          ))}
        </div>
        {self ? <p className="text-secondary">{t('users.account.selfLevel')}</p> : (
          <div className="btn-group">
            <button type="button" className="btn btn-primary btn-sm" disabled={busy || role === user.role} onClick={saveRole}>{t('users.account.saveLevel')}</button>
          </div>
        )}
        <p className="text-secondary">{t('users.account.levelNote')}</p>
      </section>

      <section className="users-card">
        <h4>{t('users.account.resetPassword')}</h4>
        {problem && <p role="alert" className="message">{problem}</p>}
        <div className="users-row">
          <label>{t('users.account.newPassword')}
            <input className="filter-input" type="password" autoComplete="new-password" value={password} onChange={e => setPassword(e.target.value)} />
          </label>
          <label>{t('users.confirmPassword')}
            <input className="filter-input" type="password" autoComplete="new-password" value={confirm} onChange={e => setConfirm(e.target.value)} />
          </label>
        </div>
        <div className="btn-group">
          <button type="button" className="btn btn-secondary btn-sm" disabled={busy || !password} onClick={savePassword}>{t('users.account.changePassword')}</button>
        </div>
        <p className="text-secondary">{t('users.account.passwordNote')}</p>
      </section>

      {user.status === 'active' && (
        <section className="users-card">
          <h4>{t('users.account.external')}</h4>
          <p className="text-secondary">{t('users.account.externalHint')}</p>
          <div className="btn-group">
            <button type="button" className="btn btn-secondary btn-sm" onClick={() => setLinking(true)}>{t('users.account.linkRancher')}</button>
          </div>
          {linking && <ExternalIdentityEditor key={user.id} user={user} onClose={() => setLinking(false)} />}
        </section>
      )}
    </div>
  );
}
