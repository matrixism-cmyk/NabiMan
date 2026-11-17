import React, { useState } from 'react';
import { apiPost, useApi } from '../shared/api/useApi';
import { CreationOptionsJSON, createPasskey, passkeyAlreadyThere, passkeyCancelled, passkeysSupported } from '../shared/auth/passkey';
import { useT } from '../i18n';

interface Passkey { id: string; name: string; created_at: string; last_used: string | null; }
interface PasskeyList { enabled: boolean; reason: string | null; local_password_available: boolean; limit: number; passkeys: Passkey[]; }
interface Begun { challenge_id: string; publicKey: CreationOptionsJSON; }
type Row = { id: string; mode: 'rename' | 'delete'; value: string } | null;

/** The account's own passkeys: register this device, rename, delete. */
export default function PasskeyPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<PasskeyList>('/api/auth/passkeys');
  const [name, setName] = useState(''); const [password, setPassword] = useState('');
  const [row, setRow] = useState<Row>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ text: string; failed: boolean } | null>(null);
  const supported = passkeysSupported();

  // The server's own words for a wrong password are English; say it in the page's language.
  const said = (text?: string) => (text === 'Invalid password' ? t('passkey.wrongPassword') : text || t('common.error'));
  const attempt = async (work: () => Promise<string>) => {
    setBusy(true); setMessage(null);
    try {
      setMessage({ text: await work(), failed: false });
      refetch();
    } catch (cause) {
      const text = passkeyCancelled(cause) ? t('passkey.cancelled')
        : passkeyAlreadyThere(cause) ? t('passkey.alreadyThere')
        : cause instanceof Error && cause.message ? cause.message : t('common.error');
      setMessage({ text, failed: true });
    } finally { setBusy(false); }
  };

  const register = () => attempt(async () => {
    const begun = await apiPost<Begun>('/api/auth/passkeys/register/begin', { password });
    if (!begun.success || !begun.data) throw new Error(said(begun.message));
    const credential = await createPasskey(begun.data.publicKey);
    const done = await apiPost('/api/auth/passkeys/register/finish', { challenge_id: begun.data.challenge_id, name, credential });
    if (!done.success) throw new Error(said(done.message));
    setName(''); setPassword('');
    return t('passkey.registered');
  });
  const confirmRow = () => attempt(async () => {
    if (!row) return '';
    const done = row.mode === 'rename'
      ? await apiPost('/api/auth/passkeys/rename', { id: row.id, name: row.value })
      : await apiPost('/api/auth/passkeys/delete', { id: row.id, password: row.value });
    if (!done.success) throw new Error(said(done.message));
    setRow(null);
    return t(row.mode === 'rename' ? 'passkey.renamed' : 'passkey.deleted');
  });

  if (loading) return <div className="panel">{t('common.loading')}</div>;
  if (error || !data) return <div className="panel" role="alert">{error || t('common.error')}</div>;
  const full = data.passkeys.length >= data.limit;
  return <section className="panel" aria-label={t('passkey.title')}>
    <h2>{t('passkey.title')}</h2>
    <p>{t('passkey.hint')}</p>
    {!data.enabled && <p role="status">{t('passkey.notConfigured')}</p>}
    {!data.enabled && data.reason && <p>{t('passkey.offAdmin')} {t('passkey.offReason', { reason: data.reason })}</p>}
    {message && <p role={message.failed ? 'alert' : 'status'}>{message.text}</p>}

    <h3>{t('passkey.list')}</h3>
    {data.passkeys.length === 0 ? <p>{t('passkey.none')}</p> : <table className="data-table"><thead><tr>
      <th>{t('passkey.name')}</th><th>{t('passkey.created')}</th><th>{t('passkey.lastUsed')}</th><th>{t('common.actions')}</th>
    </tr></thead><tbody>{data.passkeys.map(passkey => <tr key={passkey.id}>
      <td>{passkey.name}</td><td>{passkey.created_at}</td><td>{passkey.last_used || t('passkey.neverUsed')}</td>
      <td><div className="btn-group">
        <button type="button" className="btn btn-sm btn-secondary" disabled={busy} onClick={() => setRow({ id: passkey.id, mode: 'rename', value: passkey.name })}>{t('passkey.rename')}</button>
        <button type="button" className="btn btn-sm btn-danger" disabled={busy} onClick={() => setRow({ id: passkey.id, mode: 'delete', value: '' })}>{t('common.delete')}</button>
      </div></td>
    </tr>)}</tbody></table>}

    {row && <form style={{ maxWidth: 500, textAlign: 'left' }} onSubmit={e => { e.preventDefault(); confirmRow(); }}>
      {row.mode === 'delete' && <p>{t('passkey.deleteAsk', { name: data.passkeys.find(p => p.id === row.id)?.name || '' })}</p>}
      <label style={{ display: 'block', margin: '16px 0' }}>{t(row.mode === 'rename' ? 'passkey.name' : 'app.currentPassword')}
        {row.mode === 'rename'
          ? <input aria-label={t('passkey.name')} value={row.value} maxLength={64} required autoFocus onChange={e => setRow({ ...row, value: e.target.value })} style={{ display: 'block', width: '100%' }} />
          : <input aria-label={t('app.currentPassword')} type="password" autoComplete="current-password" value={row.value} required autoFocus onChange={e => setRow({ ...row, value: e.target.value })} style={{ display: 'block', width: '100%' }} />}
      </label>
      <div className="btn-group">
        <button className={`btn ${row.mode === 'delete' ? 'btn-danger' : 'btn-primary'}`} type="submit" disabled={busy}>{t(row.mode === 'rename' ? 'common.save' : 'common.delete')}</button>
        <button className="btn btn-secondary" type="button" disabled={busy} onClick={() => setRow(null)}>{t('common.cancel')}</button>
      </div>
    </form>}

    {!data.local_password_available ? <p>{t('passkey.external')}</p>
      : !supported ? <p role="status">{t('passkey.unsupported')}</p>
      : data.enabled && !row && <form style={{ maxWidth: 500, textAlign: 'left' }} onSubmit={e => { e.preventDefault(); register(); }}>
        {full && <p role="status">{t('passkey.limit', { count: data.limit })}</p>}
        <label style={{ display: 'block', margin: '16px 0' }}>{t('passkey.name')}
          <input aria-label={t('passkey.name')} value={name} maxLength={64} required placeholder={t('passkey.namePlaceholder')} onChange={e => setName(e.target.value)} style={{ display: 'block', width: '100%' }} />
        </label>
        <label style={{ display: 'block', margin: '16px 0' }}>{t('app.currentPassword')}
          <input aria-label={t('app.currentPassword')} type="password" autoComplete="current-password" value={password} required onChange={e => setPassword(e.target.value)} style={{ display: 'block', width: '100%' }} />
        </label>
        <button className="btn btn-primary" type="submit" disabled={busy || full}>{t(busy ? 'passkey.waiting' : 'passkey.register')}</button>
      </form>}
  </section>;
}
