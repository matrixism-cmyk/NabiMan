import React, { useState } from 'react';
import './encrypted-backup.css';
import { apiPost } from '../shared/api/useApi';
import { useT } from '../i18n';
export default function EncryptedBackupPanel({ files, refresh }: { files: string[]; refresh: () => void }) {
  const { t } = useT();
  const [paths, setPaths] = useState(''); const [password, setPassword] = useState(''); const [repeat, setRepeat] = useState('');
  const [filename, setFilename] = useState(''); const [target, setTarget] = useState(''); const [restorePassword, setRestorePassword] = useState('');
  const [acknowledged, setAcknowledged] = useState(false); const [busy, setBusy] = useState(false); const [message, setMessage] = useState(''); const [error, setError] = useState('');
  const execute = async (restore: boolean) => {
    setBusy(true); setError(''); setMessage('');
    const body = restore ? { filename, target_dir: target, password: restorePassword } : { paths: paths.split(',').map(p => p.trim()).filter(Boolean), password };
    setPassword(''); setRepeat(''); setRestorePassword(''); setAcknowledged(false);
    try {
      const result = await apiPost<string>(restore ? '/api/backup/restore-encrypted' : '/api/backup/create-encrypted', body);
      if (!result.success) throw new Error(result.message);
      setMessage(restore ? t('encrypted.restored') : t('encrypted.created') + ': ' + result.data);
      refresh();
    } catch (cause) { setError(cause instanceof Error ? cause.message : t('common.error')); }
    finally { setBusy(false); }
  };
  return <section className="encrypted-backup" aria-label={t('encrypted.title')}>
    <h3>{t('encrypted.title')}</h3><p>{t('encrypted.description')}</p><p>{t('encrypted.limits')}</p>
    <fieldset disabled={busy}>
      <legend>{t('encrypted.create')}</legend>
      <label>{t('backup.paths')}<input aria-label={t('encrypted.paths')} value={paths} onChange={e => setPaths(e.target.value)} /></label>
      <label>{t('encrypted.password')}<input type="password" autoComplete="new-password" value={password} onChange={e => setPassword(e.target.value)} /></label>
      <label>{t('encrypted.repeat')}<input type="password" autoComplete="new-password" value={repeat} onChange={e => setRepeat(e.target.value)} /></label>
      <button className="btn btn-primary" disabled={!paths.trim() || Array.from(password).length < 12 || password !== repeat} onClick={() => execute(false)}>{t('encrypted.create')}</button>
    </fieldset>
    <fieldset disabled={busy}>
      <legend>{t('encrypted.restore')}</legend>
      <label>{t('encrypted.file')}<select aria-label={t('encrypted.file')} value={filename} onChange={e => { setFilename(e.target.value); setAcknowledged(false); }}><option value="">—</option>{files.filter(f => f.endsWith('.tar.gz.age')).map(f => <option key={f} value={f}>{f}</option>)}</select></label>
      <label>{t('encrypted.target')}<input value={target} onChange={e => { setTarget(e.target.value); setAcknowledged(false); }} placeholder="/srv/recovery/new-directory" /></label>
      <label>{t('encrypted.restorePassword')}<input type="password" autoComplete="off" value={restorePassword} onChange={e => setRestorePassword(e.target.value)} /></label>
      <label><input type="checkbox" checked={acknowledged} onChange={e => setAcknowledged(e.target.checked)} />{t('encrypted.acknowledge')}</label>
      <button className="btn btn-primary" disabled={!filename || !target.trim() || Array.from(restorePassword).length < 12 || !acknowledged} onClick={() => execute(true)}>{t('encrypted.restore')}</button>
    </fieldset>
    {busy && <p role="status">{t('encrypted.busy')}</p>}{message && <p role="status">{message}</p>}{error && <p role="alert">{error}</p>}
    <p>{t('encrypted.legacy')}</p>
  </section>;
}
