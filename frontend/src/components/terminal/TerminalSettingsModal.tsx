import React, { useState } from 'react';
import { useT } from '../../i18n';
import { DEFAULT_TERMINAL_SETTINGS, TerminalSettings, useTerminalSettings } from './settings';

const SCROLLBACK_PRESETS = [1000, 5000, 10000, 50000, 100000];
const TIMEOUT_PRESETS: { label: string; value: number }[] = [
  { label: '30m', value: 1800 },
  { label: '1h', value: 3600 },
  { label: '6h', value: 21600 },
  { label: '24h', value: 86400 },
  { label: '7d', value: 604800 },
];

/** Terminal preferences: history size, keep-alive, and text size. */
export default function TerminalSettingsModal({ onClose }: { onClose: () => void }) {
  const { t } = useT();
  const { settings, save } = useTerminalSettings();
  const [draft, setDraft] = useState<TerminalSettings>(settings);
  const [message, setMessage] = useState('');
  const [saving, setSaving] = useState(false);

  const set = <K extends keyof TerminalSettings>(key: K, value: TerminalSettings[K]) =>
    setDraft((d) => ({ ...d, [key]: value }));

  const handleSave = async () => {
    setSaving(true);
    const error = await save({
      ...draft,
      scrollback_lines: Math.min(200000, Math.max(100, Math.floor(draft.scrollback_lines) || 5000)),
      font_size: Math.min(28, Math.max(8, Math.floor(draft.font_size) || 14)),
      keepalive_interval_secs: Math.min(3600, Math.max(5, Math.floor(draft.keepalive_interval_secs) || 30)),
    });
    setSaving(false);
    if (error) { setMessage(error); return; }
    onClose();
  };

  return (
    <div className="modal-overlay term-settings-overlay" onClick={onClose}>
      <div className="modal-content term-settings" onClick={(e) => e.stopPropagation()}>
        <h3>{t('terminal.settings')}</h3>
        {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}

        <div className="form-group">
          <label>{t('terminal.scrollbackLines')}</label>
          <div className="filter-row">
            <input
              type="number" min={100} max={200000} step={100}
              className="filter-input" style={{ width: 140 }}
              value={draft.scrollback_lines}
              onChange={(e) => set('scrollback_lines', parseInt(e.target.value, 10) || 0)}
            />
            {SCROLLBACK_PRESETS.map((n) => (
              <button
                key={n} type="button"
                className={`btn btn-sm ${draft.scrollback_lines === n ? 'btn-primary' : 'btn-secondary'}`}
                onClick={() => set('scrollback_lines', n)}
              >{n.toLocaleString()}</button>
            ))}
          </div>
          <small className="text-secondary">{t('terminal.scrollbackHint')}</small>
        </div>

        <div className="form-group">
          <label>
            <input
              type="checkbox" checked={draft.keepalive}
              onChange={(e) => set('keepalive', e.target.checked)}
            />{' '}
            {t('terminal.keepalive')}
          </label>
          <small className="text-secondary">{t('terminal.keepaliveHint')}</small>
        </div>

        <div className="form-group">
          <label>{t('terminal.keepaliveInterval')}</label>
          <input
            type="number" min={5} max={3600} className="filter-input" style={{ width: 140 }}
            value={draft.keepalive_interval_secs}
            onChange={(e) => set('keepalive_interval_secs', parseInt(e.target.value, 10) || 0)}
          />
          <small className="text-secondary">{t('terminal.keepaliveIntervalHint')}</small>
        </div>

        {!draft.keepalive && (
          <div className="form-group">
            <label>{t('terminal.idleTimeout')}</label>
            <div className="filter-row">
              <button
                type="button"
                className={`btn btn-sm ${draft.idle_timeout_secs === 0 ? 'btn-primary' : 'btn-secondary'}`}
                onClick={() => set('idle_timeout_secs', 0)}
              >{t('terminal.permanent')}</button>
              {TIMEOUT_PRESETS.map((p) => (
                <button
                  key={p.value} type="button"
                  className={`btn btn-sm ${draft.idle_timeout_secs === p.value ? 'btn-primary' : 'btn-secondary'}`}
                  onClick={() => set('idle_timeout_secs', p.value)}
                >{p.label}</button>
              ))}
            </div>
            <small className="text-secondary">{t('terminal.idleTimeoutHint')}</small>
          </div>
        )}

        <div className="form-group">
          <label>
            <input
              type="checkbox" checked={draft.restore_scrollback}
              onChange={(e) => set('restore_scrollback', e.target.checked)}
            />{' '}
            {t('terminal.restoreScrollback')}
          </label>
          <small className="text-secondary">{t('terminal.restoreScrollbackHint')}</small>
        </div>

        <div className="form-group">
          <label>{t('terminal.fontSize')}</label>
          <input
            type="number" min={8} max={28} className="filter-input" style={{ width: 140 }}
            value={draft.font_size}
            onChange={(e) => set('font_size', parseInt(e.target.value, 10) || 0)}
          />
        </div>

        <div className="btn-group">
          <button className="btn btn-primary" onClick={handleSave} disabled={saving}>
            {saving ? t('common.working') : t('common.save')}
          </button>
          <button className="btn btn-secondary" onClick={() => setDraft(DEFAULT_TERMINAL_SETTINGS)}>
            {t('terminal.resetDefaults')}
          </button>
          <button className="btn btn-secondary" onClick={onClose}>{t('common.cancel')}</button>
        </div>
      </div>
    </div>
  );
}
