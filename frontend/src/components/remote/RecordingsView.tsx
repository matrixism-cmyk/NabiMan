import React, { useEffect, useState } from 'react';
import { useT } from '../../shared/i18n';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import RecordingPlayer from './RecordingPlayer';
import { deleteRecording, getSettings, listRecordings, Recording, RecordingSettings, saveSettings } from './recordingsApi';

function size(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
const when = (ms: number) => (ms ? new Date(ms).toLocaleString() : '—');

/** The super administrator's session recordings: the on/off policy, and the
 *  list with a player. Everything here is off until an administrator turns it on. */
export default function RecordingsView() {
  const { t } = useT();
  const [settings, setSettings] = useState<RecordingSettings | null>(null);
  const [rows, setRows] = useState<Recording[]>([]);
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const [playing, setPlaying] = useState<Recording | null>(null);

  const reload = () => { listRecordings().then((r) => { if (r.success && r.data) setRows(r.data); }); };
  useEffect(() => {
    getSettings().then((r) => { if (r.success && r.data) setSettings(r.data); });
    reload();
  }, []);

  const save = async (next: RecordingSettings) => {
    setSettings(next); setBusy(true);
    const r = await saveSettings(next);
    setBusy(false);
    setMessage(r.success ? t('remote.rec.saved') : r.message || t('common.error'));
  };
  const set = <K extends keyof RecordingSettings>(key: K, value: RecordingSettings[K]) => {
    if (settings) save({ ...settings, [key]: value });
  };
  const remove = async (rec: Recording) => {
    if (!(await confirmDialog({ message: t('remote.rec.deleteConfirm', { label: rec.label }), danger: true, confirmLabel: t('common.delete') }))) return;
    await deleteRecording(rec.sid);
    reload();
  };

  return (
    <div className="panel rec-view">
      <div className="panel-header"><h2>{t('remote.rec.title')}</h2></div>
      <p className="text-secondary">{t('remote.rec.intro')}</p>

      {settings && (
        <section className="rec-settings">
          <label className={`rec-toggle ${settings.enabled ? 'is-on' : ''}`}>
            <input type="checkbox" checked={settings.enabled} disabled={busy} onChange={(e) => set('enabled', e.target.checked)} />
            <span><strong>{t('remote.rec.enable')}</strong><small>{t('remote.rec.enableHint')}</small></span>
          </label>
          <label className="rec-toggle">
            <input type="checkbox" checked={settings.keystrokes} disabled={busy || !settings.enabled} onChange={(e) => set('keystrokes', e.target.checked)} />
            <span><strong>{t('remote.rec.keystrokes')}</strong><small>{t('remote.rec.keystrokesHint')}</small></span>
          </label>
          <div className="rec-nums">
            <label>{t('remote.rec.retention')}
              <input type="number" min={0} max={3650} className="filter-input" value={settings.retention_days} disabled={busy}
                onChange={(e) => setSettings({ ...settings, retention_days: Math.max(0, Number(e.target.value) || 0) })}
                onBlur={() => save(settings)} />
            </label>
            <label>{t('remote.rec.maxmb')}
              <input type="number" min={1} max={2048} className="filter-input" value={settings.max_mb} disabled={busy}
                onChange={(e) => setSettings({ ...settings, max_mb: Math.max(1, Number(e.target.value) || 1) })}
                onBlur={() => save(settings)} />
            </label>
          </div>
          <p className="text-secondary rec-note">{t('remote.rec.newOnly')}</p>
        </section>
      )}
      {message && <div role="status" className="message" onClick={() => setMessage('')}>{message}</div>}

      <h3 className="rec-list-head">{t('remote.rec.listTitle')} <em>{rows.length}</em>
        <button type="button" className="btn btn-secondary btn-sm" onClick={reload}>⟳</button>
      </h3>
      {rows.length === 0 ? <p className="text-secondary">{t('remote.rec.empty')}</p> : (
        <table className="data-table rec-table">
          <thead><tr><th>{t('remote.rec.started')}</th><th>{t('remote.rec.session')}</th><th>{t('remote.rec.owner')}</th><th>{t('remote.rec.size')}</th><th /></tr></thead>
          <tbody>{rows.map((r) => (
            <tr key={r.sid}>
              <td>{when(r.started_ms)}</td>
              <td><strong>{r.label}</strong>{r.live && <span className="rec-live">{t('remote.rec.live')}</span>}</td>
              <td>{r.owner}</td>
              <td>{size(r.bytes)}</td>
              <td className="rec-actions">
                <button type="button" className="btn btn-primary btn-sm" onClick={() => setPlaying(r)}>▶ {t('remote.rec.play')}</button>
                <button type="button" className="btn btn-danger btn-sm" onClick={() => remove(r)}>{t('common.delete')}</button>
              </td>
            </tr>
          ))}</tbody>
        </table>
      )}
      {playing && <RecordingPlayer sid={playing.sid} label={playing.label} onClose={() => setPlaying(null)} />}
    </div>
  );
}
