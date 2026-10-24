import React, { useEffect, useState } from 'react';
import Modal from '../../shared/ui/Modal';
import { useApi, apiRequest } from '../../shared/api/useApi';
import { useT } from '../../shared/i18n';

interface User { id: string; username: string; role: string }

/** Who, besides administrators, may open a given VM console. Admins pick the
 *  accounts here; the grant is enforced server-side per console. */
export default function ConsoleAccessModal({ console: key, label, onClose }: {
  console: string; label: string; onClose: () => void;
}) {
  const { t } = useT();
  const { data: users } = useApi<User[]>('/api/users');
  const { data: granted } = useApi<string[]>(`/api/remote/consoles/${encodeURIComponent(key)}/access`);
  const [chosen, setChosen] = useState<Set<string> | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');

  useEffect(() => { if (granted && chosen === null) setChosen(new Set(granted)); }, [granted, chosen]);

  // Administrators always have access, so they are not offered here.
  const pickable = (users || []).filter((u) => u.role !== 'admin');
  const toggle = (id: string) => {
    const next = new Set(chosen); if (next.has(id)) next.delete(id); else next.add(id); setChosen(next);
  };
  const save = async () => {
    setBusy(true); setMessage('');
    const res = await apiRequest(`/api/remote/consoles/${encodeURIComponent(key)}/access`, {
      method: 'PUT', body: JSON.stringify({ users: Array.from(chosen || []) }),
    });
    setBusy(false);
    if (res.success) onClose(); else setMessage(res.message || t('common.error'));
  };

  return (
    <Modal className="console-access" onClose={onClose} title={`${t('remote.console.access.title')} · ${label}`}>
      <p className="text-secondary" style={{ fontSize: 13 }}>{t('remote.console.access.intro')}</p>
      {chosen === null ? <p>{t('common.loading')}</p> : (
        <div className="console-access-list" style={{ maxHeight: 320, overflowY: 'auto', margin: '8px 0' }}>
          {pickable.length === 0 && <p className="text-secondary">{t('remote.console.access.none')}</p>}
          {pickable.map((u) => (
            <label key={u.id} style={{ display: 'flex', gap: 8, alignItems: 'center', padding: '4px 0' }}>
              <input type="checkbox" checked={chosen.has(u.id)} onChange={() => toggle(u.id)} />
              <span>{u.username}</span>
              <span className={`users-role role-${u.role}`} style={{ fontSize: 11 }}>{u.role}</span>
            </label>
          ))}
        </div>
      )}
      {message && <p role="alert" className="message">{message}</p>}
      <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
        <button className="btn btn-secondary btn-sm" onClick={onClose}>{t('common.cancel')}</button>
        <button className="btn btn-primary btn-sm" onClick={save} disabled={busy || chosen === null}>{t('common.save')}</button>
      </div>
    </Modal>
  );
}
