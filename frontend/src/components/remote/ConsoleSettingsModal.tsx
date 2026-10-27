import React, { useEffect, useState } from 'react';
import Modal from '../../shared/ui/Modal';
import { useT } from '../../shared/i18n';
import PauseField from '../terminal/dictation/PauseField';
import { dictationSetup } from '../terminal/dictation/sockets';
import { ConsoleShare, ConsoleViewPrefs, RESOLUTIONS } from './consoleView';

/** The dictation defaults a console's settings start from; null where it cannot dictate. */
interface Dictation { pause_ms: number; auto_enter: boolean; voice_lock?: boolean }

/**
 * A VM's settings, like a terminal's but without what only a terminal has
 * (fonts, scrollback, closing time): this browser's own view, never sent
 * anywhere, so whoever else has the VM open sees no change. A share link's
 * visitor gets the same dialog, minus what belongs to an account.
 */
export default function ConsoleSettingsModal({ view, onChange, onClose, rdp, share }: {
  view: ConsoleViewPrefs;
  onChange: (next: ConsoleViewPrefs) => void;
  onClose: () => void;
  /** Display and touch settings are RDP's; a VNC console scales itself. */
  rdp?: boolean;
  share?: ConsoleShare;
}) {
  const { t } = useT();
  const [account, setAccount] = useState<Dictation | null>(null);
  useEffect(() => {
    if (share) return undefined;
    let live = true;
    dictationSetup().then((s) => { if (live && s.available) setAccount({ pause_ms: s.endpoint_ms, auto_enter: s.auto_enter, voice_lock: s.voice_lock }); });
    return () => { live = false; };
  }, [share]);
  const dictation: Dictation | null = share ? (share.viewOnly ? null : share.dictation || null) : account;
  const set = (patch: ConsoleViewPrefs) => onChange({ ...view, ...patch });

  return (
    <Modal className="share-dialog console-settings" onClose={onClose} title={t('remote.console.settings')}>
      <p className="text-secondary">{t('remote.console.settingsHint')}</p>
      <div className="form-group">
        <label>
          <input type="checkbox" checked={view.auto_reconnect !== false} onChange={(e) => set({ auto_reconnect: e.target.checked })} />{' '}
          {t('remote.console.keepalive')}
        </label>
        <small className="text-secondary">{t('remote.console.keepaliveHint')}</small>
      </div>
      {rdp && (<>
        <div className="form-group">
          <label htmlFor="console-res">{t('remote.console.resolution')}</label>
          <select id="console-res" className="select-input" value={RESOLUTIONS.includes(view.resolution || '') ? view.resolution : 'fit'}
            onChange={(e) => set({ resolution: e.target.value === 'fit' ? undefined : e.target.value })}>
            <option value="fit">{t('remote.console.resolutionFit')}</option>
            {RESOLUTIONS.map((r) => <option key={r} value={r}>{r.replace('x', ' × ')}</option>)}
          </select>
          <small className="text-secondary">{t('remote.console.resolutionHint')}</small>
        </div>
        {!share?.viewOnly && (
          <div className="form-group">
            <label htmlFor="console-touch">{t('remote.console.touch')}</label>
            <select id="console-touch" className="select-input" value={view.touch || 'screen'}
              onChange={(e) => set({ touch: e.target.value === 'pad' ? 'pad' : undefined })}>
              <option value="screen">{t('remote.console.touchScreen')}</option>
              <option value="pad">{t('remote.console.touchPad')}</option>
            </select>
            <small className="text-secondary">{t('remote.console.touchHint')}</small>
          </div>
        )}
      </>)}
      {dictation && (<>
        <PauseField id="console-pause" value={view.dictation_pause_ms ?? dictation.pause_ms}
          onChange={(ms) => set({ dictation_pause_ms: ms })} />
        <div className="form-group">
          <label>
            <input type="checkbox" checked={view.dictation_auto_enter ?? dictation.auto_enter}
              onChange={(e) => set({ dictation_auto_enter: e.target.checked })} />{' '}
            {t('dictation.autoEnter')}
          </label>
          <small className="text-secondary">{t('remote.console.autoEnterHint')}</small>
        </div>
        {!share && (
          <div className="form-group">
            <label>
              <input type="checkbox" checked={view.dictation_voice_lock ?? Boolean(dictation.voice_lock)}
                onChange={(e) => set({ dictation_voice_lock: e.target.checked })} />{' '}
              {t('dictation.voiceLock')}
            </label>
            <small className="text-secondary">{t('dictation.voiceLockSettingHint')}</small>
          </div>
        )}
      </>)}
      <div className="btn-group">
        <button type="button" className="btn btn-secondary" onClick={() => onChange({})}>{t('share.viewReset')}</button>
        <button type="button" className="btn btn-primary" onClick={onClose}>{t('common.close')}</button>
      </div>
    </Modal>
  );
}
