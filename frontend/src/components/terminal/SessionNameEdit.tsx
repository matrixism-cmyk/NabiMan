import React, { useEffect, useState } from 'react';
import { apiPost } from '../../hooks/useApi';
import { useT } from '../../i18n';

interface Props {
  sessionId: string;
  /** The name it has now; empty means it goes by its label. */
  name: string;
  /** Called with the saved name, so the caller can show it or refetch. */
  onRenamed?: (name: string) => void;
  /** `bar` is the compact form for a floating window's title bar. */
  variant?: 'head' | 'bar';
}

/**
 * The open session's name, with a pencil to change it in place — the same
 * rename the session list offers, without going back to the list.
 */
export default function SessionNameEdit({ sessionId, name, onRenamed, variant = 'head' }: Props) {
  const { t } = useT();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');

  // Another session in the same pane is another name.
  useEffect(() => { setEditing(false); setError(''); }, [sessionId]);

  const start = () => { setDraft(name); setError(''); setEditing(true); };

  const save = async () => {
    setSaving(true);
    try {
      const res = await apiPost<string>(`/api/terminal/sessions/${sessionId}/name`, { name: draft });
      if (!res.success) { setError(res.message || t('common.error')); return; }
      setEditing(false);
      setError('');
      onRenamed?.(typeof res.data === 'string' ? res.data : draft.trim());
    } catch {
      setError(t('terminal.actionUncertain'));
    } finally {
      setSaving(false);
    }
  };

  if (editing) {
    return (
      <form className={`session-name-edit is-editing session-name-edit-${variant}`}
        onSubmit={(e) => { e.preventDefault(); save(); }}>
        <input className="filter-input" value={draft} autoFocus maxLength={60}
          aria-label={t('terminal.sessionName')}
          placeholder={t('terminal.namePlaceholder')}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); setEditing(false); } }} />
        <button className="btn btn-primary btn-sm" type="submit" disabled={saving}>{t('common.save')}</button>
        <button className="btn btn-secondary btn-sm" type="button" onClick={() => setEditing(false)}>
          {t('common.cancel')}
        </button>
        {error && <span role="alert" className="session-name-error">{error}</span>}
      </form>
    );
  }

  return (
    <span className={`session-name-edit session-name-edit-${variant}`}>
      <span className="session-name-text" title={name || undefined}>
        {name || t('terminal.unnamedSession')}
      </span>
      <button type="button" className="session-rename-btn" title={t('terminal.rename')}
        aria-label={t('terminal.rename')} onClick={(e) => { e.stopPropagation(); start(); }}>✎</button>
    </span>
  );
}
