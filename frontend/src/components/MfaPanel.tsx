import React, { useState } from 'react';
import { apiPost, clearToken, useApi } from '../hooks/useApi';
import { useT } from '../i18n';
import { confirmDialog } from './common/ConfirmDialog';
interface MfaStatus { required: boolean; verified: boolean; blocked: boolean; username: string; enabled: boolean; local_password_available: boolean; recovery_codes_remaining: number; }
interface Enrollment { secret: string; qr_png_base64: string; recovery_codes: string[]; expires_at: number; }
export default function MfaPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<MfaStatus>('/api/auth/2fa/status');
  const [password, setPassword] = useState(''); const [code, setCode] = useState('');
  const [enrollment, setEnrollment] = useState<Enrollment | null>(null);
  const [codes, setCodes] = useState<string[]>([]); const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false); const [message, setMessage] = useState('');
  const relogin = () => { clearToken(); sessionStorage.setItem('nabiman_security_changed', '1'); window.location.reload(); };
  const run = async (action: 'setup' | 'verify-setup' | 'disable' | 'recovery-codes') => {
    if ((action === 'disable' || action === 'recovery-codes') && !(await confirmDialog({ message: t(`mfa.confirm.${action}`), danger: true }))) return;
    setBusy(true); setMessage('');
    try {
      const result = await apiPost<Enrollment>(`/api/auth/2fa/${action}`,
        action === 'verify-setup' ? { code, recovery_saved: saved } : { password, code });
      if (!result.success) throw new Error(result.message || t('common.error'));
      setPassword(''); setCode('');
      if (action === 'setup') { if (!result.data) throw new Error(t('common.error')); setEnrollment(result.data); setCodes(result.data.recovery_codes); setSaved(false); }
      else if (action === 'recovery-codes') { if (!result.data) throw new Error(t('common.error')); setCodes(result.data.recovery_codes); setSaved(false); refetch(); }
      else relogin();
    } catch (cause) {
      if (!(cause instanceof DOMException && cause.name === 'AbortError')) setMessage(cause instanceof Error ? cause.message : t('common.error'));
    } finally { setBusy(false); }
  };
  const download = () => {
    const url = URL.createObjectURL(new Blob([`NabiMan - ${data?.username || ''}\n\n${codes.join('\n')}\n`], { type: 'text/plain' }));
    const a = document.createElement('a'); a.href = url; a.download = 'nabiman-recovery-codes.txt'; a.click(); URL.revokeObjectURL(url);
  };
  if (loading) return <div className="panel">{t('common.loading')}</div>;
  if (error || !data) return <div className="panel" role="alert">{error || t('common.error')}</div>;
  return <section className="panel" aria-label={t('mfa.title')}>
    <h2>{t('mfa.title')}</h2><p>{t(data.enabled ? 'mfa.enabled' : 'mfa.disabled')}</p>
    <p>{t('mfa.hint')}</p>
    {data.required && <p role="status">{t(data.local_password_available ? 'mfa.policyRequired' : 'mfa.policyExternal')}</p>}
    {data.blocked && data.enabled && <button className="btn btn-primary" onClick={relogin}>{t('login.button')}</button>}
    {message && <p role="alert">{message}</p>}
    {!data.local_password_available ? <p>{t('mfa.external')}</p> : <>
      {enrollment && <div>
        <p>{t('mfa.scan')}</p>
        <img src={`data:image/png;base64,${enrollment.qr_png_base64}`} alt={t('mfa.qr')} width="220" height="220" />
        <p>{t('mfa.secret')}: <code style={{ overflowWrap: 'anywhere' }}>{enrollment.secret}</code></p>
      </div>}
      {codes.length > 0 && <div>
        <p>{t('mfa.saveHint')}</p>
        <textarea aria-label={t('mfa.codes')} readOnly value={codes.join('\n')} rows={10} style={{ width: '100%', maxWidth: 500, textAlign: 'left' }} />
        <p><button className="btn btn-secondary" onClick={download}>{t('mfa.download')}</button></p>
        <label><input type="checkbox" checked={saved} onChange={e => setSaved(e.target.checked)} /> {t('mfa.saved')}</label>
        {!enrollment && saved && <button className="btn btn-secondary" onClick={() => setCodes([])}>{t('common.close')}</button>}
      </div>}
      {data.enabled && <p>{t('mfa.remaining').replace('{count}', String(data.recovery_codes_remaining))}</p>}
      <form style={{ maxWidth: 500, textAlign: 'left' }} onSubmit={e => { e.preventDefault(); run(enrollment ? 'verify-setup' : data.enabled ? 'disable' : 'setup'); }}>
        {!enrollment && <label style={{ display: 'block', margin: '16px 0' }}>{t('app.currentPassword')}
          <input aria-label={t('app.currentPassword')} type="password" autoComplete="current-password" value={password} onChange={e => setPassword(e.target.value)} required style={{ display: 'block', width: '100%' }} />
        </label>}
        {(enrollment || data.enabled) && <label style={{ display: 'block', margin: '16px 0' }}>{t('login.totpCode')}
          <input aria-label={t('login.totpCode')} autoComplete="one-time-code" value={code} onChange={e => setCode(e.target.value)} maxLength={64} required style={{ display: 'block', width: '100%' }} />
        </label>}
        <div className="btn-group">
          <button className="btn btn-primary" disabled={busy || !!enrollment && !saved || data.enabled && data.required} type="submit">{t(enrollment ? 'mfa.enable' : data.enabled ? 'mfa.disable' : 'mfa.setup')}</button>
          {enrollment && <button className="btn btn-secondary" type="button" disabled={busy} onClick={() => { setEnrollment(null); setCodes([]); setSaved(false); setCode(''); refetch(); }}>{t('common.cancel')}</button>}
          {data.enabled && <button className="btn btn-secondary" type="button" disabled={busy || !password || !code} onClick={() => run('recovery-codes')}>{t('mfa.regenerate')}</button>}
        </div>
      </form>
    </>}
  </section>;
}
