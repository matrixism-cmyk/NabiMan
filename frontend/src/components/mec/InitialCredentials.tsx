import React, { useState } from 'react';
import { useT } from '../../i18n';
export interface BootstrapCredentials { username: string; password: string }
export function randomPassword(length = 24): string {
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_';
  const bytes = crypto.getRandomValues(new Uint8Array(length));
  return Array.from(bytes, b => alphabet[b & 63]).join('');
}
export default function InitialCredentials({ credentials, onDone }: { credentials: BootstrapCredentials; onDone: () => void }) {
  const { t } = useT(); const [show, setShow] = useState(false);
  return <section className="panel-card" aria-label={t('mec.credentials.title')} style={{ padding: '16px', textAlign: 'left' }}>
    <h3>{t('mec.credentials.title')}</h3><p>{t('mec.credentials.notice')}</p>
    <label style={{ display: 'block', marginBottom: '12px' }}>{t('mec.credentials.username')}<input readOnly value={credentials.username} style={{ display: 'block', width: '100%' }} /></label>
    <label style={{ display: 'block', marginBottom: '12px' }}>{t('mec.credentials.password')}<input readOnly type={show ? 'text' : 'password'} autoComplete="off" value={credentials.password} style={{ display: 'block', width: '100%' }} /></label>
    <button type="button" className="btn btn-secondary" onClick={() => setShow(v => !v)}>{t(show ? 'mec.credentials.hide' : 'mec.credentials.show')}</button>{' '}
    <button type="button" className="btn btn-primary" onClick={onDone}>{t('mec.credentials.done')}</button>
  </section>;
}
