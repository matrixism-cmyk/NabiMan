import React, { useRef, useState } from 'react';
import { useApi, apiPost, apiRequest } from '../hooks/useApi';
import { CronJob } from '../types';
import { useT } from '../i18n';
interface Snapshot { user: string; uid: number; revision: string; content: string; can_write: boolean }
export default function CronPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<CronJob[]>('/api/cron');
  const capability = useApi<Snapshot>('/api/cron/snapshot?user=root');
  const [showForm, setShowForm] = useState(false);
  const [editing, setEditing] = useState(false);
  const [user, setUser] = useState('root');
  const [schedule, setSchedule] = useState('');
  const [command, setCommand] = useState('');
  const [reviewed, setReviewed] = useState<Snapshot | null>(null);
  const [selected, setSelected] = useState<CronJob | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const reviewId = useRef(0);
  const canWrite = capability.data?.can_write === true && !capability.error;
  const stale = selected && !data?.some(r => r.user === selected.user && r.id === selected.id && r.revision === selected.revision);
  const reset = () => { reviewId.current++; setReviewed(null); setSelected(null); setConfirmed(false); setShowForm(false); setEditing(false); setSchedule(''); setCommand(''); };
  const review = async () => {
    const id = ++reviewId.current; setBusy(true); setMessage(''); setReviewed(null);
    try { const r = await apiRequest<Snapshot>('/api/cron/snapshot?user=' + encodeURIComponent(user));
      if (id === reviewId.current) { if (r.success && r.data?.user === user) { if (editing && r.data.revision !== selected?.revision) setMessage(t('cron.stale')); else setReviewed(r.data); } else setMessage(r.message || t('cron.reviewFailed')); }
    } catch { setMessage(t('cron.reviewFailed')); } finally { setBusy(false); }
  };
  const change = async (endpoint: string, body: object) => {
    setBusy(true); setMessage('');
    try { const r = await apiPost<string>(endpoint, body); setMessage(r.success ? (r.data || t('cron.unknownResult')) : r.message); }
    catch { setMessage(t('cron.unknownResult')); }
    finally { reset(); setBusy(false); refetch(); capability.refetch(); }
  };
  const add = (e: React.FormEvent) => {
    e.preventDefault();
    if (!canWrite || error || !reviewed?.can_write || reviewed.user !== user || !confirmed || busy || (editing && (stale || !selected || reviewed.revision !== selected.revision))) return;
    void change(editing ? '/api/cron/update' : '/api/cron/add', { user: reviewed.user, expected_revision: reviewed.revision, schedule, command, ...(editing ? { id: selected!.id } : {}) });
  };
  if (loading) return <div className="panel loading">{t('cron.loading')}</div>;
  return <div className="panel">
    <div className="panel-header"><h2>{t('cron.title')}</h2>{canWrite && <button className="btn btn-primary" disabled={busy} onClick={() => { reset(); setShowForm(!showForm); }}>{showForm ? t('common.cancel') : t('cron.addJob')}</button>}</div>
    <p>{t('cron.scopeHelp')}</p><p>{t('cron.executionHelp')}</p>
    {message && <div role="status" className="message">{message}</div>}
    {(error || capability.error) && <div role="alert" className="error">{error || capability.error}<button onClick={() => { refetch(); capability.refetch(); }}>{t('cron.reload')}</button></div>}
    {showForm && canWrite && <form onSubmit={add} className="inline-form">
      <h3>{t(editing ? 'cron.editJob' : 'cron.newJob')}</h3>
      {editing && selected && <><p>{t('cron.editHelp')}</p><label htmlFor="cron-before">{t('cron.before')}</label><textarea id="cron-before" readOnly value={`${selected.schedule} ${selected.command}`} />{stale && <div role="alert">{t('cron.stale')}</div>}</>}
      <div className="form-row"><label htmlFor="cron-user">{t('cron.user')}</label><input id="cron-user" value={user} disabled={busy || !!reviewed || editing} onChange={e => setUser(e.target.value)} required /></div>
      {!reviewed && <button type="button" disabled={busy || !user} onClick={review}>{t('cron.review')}</button>}
      {reviewed && <><label htmlFor="cron-existing">{t('cron.existing')}</label><textarea id="cron-existing" value={reviewed.content} readOnly rows={6} />
        <div className="form-row"><label htmlFor="cron-schedule">{t('cron.schedule')}</label><input id="cron-schedule" value={schedule} onChange={e => { setSchedule(e.target.value); setConfirmed(false); }} placeholder="*/5 * * * *" required disabled={busy} /><span>{t('cron.scheduleHelp')}</span></div>
        <div className="form-row"><label htmlFor="cron-command">{t('cron.command')}</label><input id="cron-command" value={command} onChange={e => { setCommand(e.target.value); setConfirmed(false); }} required disabled={busy} maxLength={8192} /></div>
        <label htmlFor="cron-after">{t('cron.after')}</label><textarea id="cron-after" readOnly value={`${schedule} ${command}`} />
        <label><input type="checkbox" checked={confirmed} onChange={e => setConfirmed(e.target.checked)} disabled={busy} />{t('cron.confirmExecution')}</label>
        <button type="submit" className="btn btn-primary" disabled={busy || !confirmed || !reviewed.can_write || !!error || !!stale}>{t(editing ? 'common.save' : 'common.add')}</button></>}
      <button type="button" disabled={busy} onClick={reset}>{t('common.cancel')}</button>
    </form>}
    {selected && !editing && <form onSubmit={e => { e.preventDefault(); if (confirmed && !busy && !stale && !error) void change('/api/cron/delete', { user: selected.user, id: selected.id, expected_revision: selected.revision }); }} className="inline-form">
      <p>{selected.user} · {selected.schedule}</p><pre>{selected.command}</pre><p>{t('cron.deleteHelp')}</p>
      {stale && <div role="alert">{t('cron.stale')}</div>}
      <label><input type="checkbox" checked={confirmed} onChange={e => setConfirmed(e.target.checked)} disabled={busy} />{t('cron.confirmDelete')}</label>
      <button type="submit" className="btn btn-danger" disabled={busy || !confirmed || !!stale || !!error || !canWrite}>{t('common.delete')}</button><button type="button" disabled={busy} onClick={reset}>{t('common.cancel')}</button>
    </form>}
    {!error && <><table className="data-table"><thead><tr><th>#</th><th>{t('cron.user')}</th><th>{t('cron.schedule')}</th><th>{t('cron.command')}</th><th>{t('common.actions')}</th></tr></thead>
      <tbody>{(data || []).map(job => <tr key={`${job.user}-${job.id}`}><td>{job.id}</td><td>{job.user}</td><td><code>{job.schedule}</code></td><td><code>{job.command}</code></td><td>{canWrite && job.can_manage && <><button className="btn btn-sm" disabled={busy} onClick={() => { reset(); setSelected({ ...job }); setUser(job.user); setSchedule(job.schedule); setCommand(job.command); setEditing(true); setShowForm(true); }}>{t('common.edit')}</button><button className="btn btn-danger btn-sm" disabled={busy} onClick={() => { reset(); setSelected({ ...job }); }}>{t('common.delete')}</button></>}</td></tr>)}</tbody></table>
      {!data?.length && <p>{t('cron.noJobs')}</p>}</>}
  </div>;
}
