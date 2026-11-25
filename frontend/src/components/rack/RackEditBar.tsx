import React from 'react';
import { useT } from '../../i18n';

interface Props {
  /** Devices that are somewhere else than in the saved layout. */
  count: number;
  canUndo: boolean;
  saving: boolean;
  /** Why the last save failed: the server's code and its own words. */
  error: { code: string; detail: string } | null;
  /** The version somebody else saved meanwhile. */
  conflict: string | null;
  /** The last thing done, for screen readers. */
  said: string;
  onSave: () => void;
  onUndo: () => void;
  onCancel: () => void;
  onReload: () => void;
}

const KNOWN = ['not_configured', 'layout_unusable', 'write_failed', 'invalid_move'];

/** Over the racks while positions are edited: that nothing is saved yet, and the ways out. */
export default function RackEditBar({ count, canUndo, saving, error, conflict, said, onSave, onUndo, onCancel, onReload }: Props) {
  const { t } = useT();
  return (
    <div className="rack-edit-bar" data-editing>
      <div className="rack-edit-row">
        <strong className="rack-edit-title" role="status">✎ {t('rack.edit.banner')}</strong>
        <span className="rack-edit-hint">{t('rack.edit.hint')}</span>
        <span className="rack-edit-actions">
          <button type="button" className="btn btn-secondary btn-sm" onClick={onUndo} disabled={!canUndo || saving}>{t('rack.edit.undo')}</button>
          <button type="button" className="btn btn-secondary btn-sm" onClick={onCancel} disabled={saving}>{t('rack.edit.cancel')}</button>
          <button type="button" className="btn btn-primary btn-sm" onClick={onSave} disabled={count === 0 || saving || conflict !== null} data-save>
            {saving ? t('rack.edit.saving') : count > 0 ? t('rack.edit.saveCount', { count }) : t('rack.edit.save')}
          </button>
        </span>
      </div>
      {conflict !== null && (
        <div className="rack-banner tone-error" role="alert" data-conflict>
          <span>{t('rack.edit.conflict', { version: conflict })}</span>
          <button type="button" className="btn btn-secondary btn-sm" onClick={onReload}>{t('rack.edit.reload')}</button>
        </div>
      )}
      {error && conflict === null && (
        <div className="rack-banner tone-error" role="alert" data-save-error>
          <strong>{t(`rack.edit.error.${KNOWN.includes(error.code) ? error.code : 'other'}`)}</strong>
          {error.detail && <code>{error.detail}</code>}
        </div>
      )}
      <span className="rack-sr-only" aria-live="polite" data-said>{said}</span>
    </div>
  );
}
