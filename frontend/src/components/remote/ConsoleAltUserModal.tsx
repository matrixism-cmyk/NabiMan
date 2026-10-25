import React, { useState } from 'react';
import Modal from '../../shared/ui/Modal';
import { useT } from '../../shared/i18n';

/**
 * Connect this console window as a different account — for a multi-user VM that
 * gives each user an independent session. The password stays in this window's
 * memory (re-sent on reconnect) and is never stored.
 */
export default function ConsoleAltUserModal({ onConnect, onClose }: {
  onConnect: (user: string, password: string) => void;
  onClose: () => void;
}) {
  const { t } = useT();
  const [user, setUser] = useState('');
  const [password, setPassword] = useState('');
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!user.trim()) return;
    onConnect(user.trim(), password);
  };
  return (
    <Modal className="share-dialog" onClose={onClose} title={t('remote.console.altUser')}>
      <p className="text-secondary">{t('remote.console.altUserHint')}</p>
      <form className="rw-form" onSubmit={submit}>
        <div className="form-group">
          <label>{t('remote.console.form.user')}</label>
          <input className="filter-input" value={user} autoFocus autoComplete="off"
            placeholder="user" onChange={(e) => setUser(e.target.value)} />
        </div>
        <div className="form-group">
          <label>{t('remote.console.form.pw')}</label>
          <input className="filter-input" type="password" value={password} autoComplete="new-password"
            onChange={(e) => setPassword(e.target.value)} />
        </div>
        <div className="btn-group">
          <button type="submit" className="btn btn-primary" disabled={!user.trim()}>{t('common.connect')}</button>
          <button type="button" className="btn btn-secondary" onClick={onClose}>{t('common.cancel')}</button>
        </div>
      </form>
    </Modal>
  );
}
