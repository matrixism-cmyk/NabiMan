import React, { useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useT } from '../../../i18n';
import { AttrsResult, changeAttrs, isFolder, modeString, SftpAttrsCaps, SftpEntry, SftpSource } from './sftpApi';

interface Props {
  source: SftpSource;
  /** The entries to change, with their full paths. */
  targets: { entry: SftpEntry; path: string }[];
  caps: SftpAttrsCaps;
  onDone: (result: AttrsResult) => void;
  onClose: () => void;
}

const WHO = ['owner', 'group', 'others'] as const;
const WHAT = [['read', 4], ['write', 2], ['exec', 1]] as const;

/** The value every target shares, or undefined when they differ. */
function common<T>(values: T[]): T | undefined {
  return values.every((v) => v === values[0]) ? values[0] : undefined;
}

const nameOf = (e: SftpEntry, which: 'owner' | 'group') =>
  (which === 'owner' ? e.owner ?? e.uid : e.group ?? e.gid)?.toString() ?? '';

/**
 * Permissions, owner and group for one entry or many. Only what is changed is
 * sent: an owner box left empty keeps every owner, and the permissions are
 * only touched when "change permissions" is ticked — on by default when all
 * the chosen entries share a mode. Folders can take everything inside along.
 */
export default function AttrsDialog({ source, targets, caps, onDone, onClose }: Props) {
  const { t } = useT();
  const titleId = useId();
  const listId = useId();
  const dialogRef = useRef<HTMLFormElement>(null);
  const shared = common(targets.map((x) => x.entry.mode));
  const [setMode, setSetMode] = useState(shared !== undefined && shared !== null);
  const [mode, setModeValue] = useState<number>(shared ?? 0o644);
  const [octal, setOctal] = useState((shared ?? 0o644).toString(8).padStart(3, '0'));
  const [owner, setOwner] = useState('');
  const [group, setGroup] = useState('');
  const [recursive, setRecursive] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const folders = targets.some((x) => isFolder(x.entry));
  const currentOwner = common(targets.map((x) => nameOf(x.entry, 'owner')));
  const currentGroup = common(targets.map((x) => nameOf(x.entry, 'group')));

  useEffect(() => {
    dialogRef.current?.querySelector<HTMLElement>('input')?.focus();
  }, []);

  const toggle = (bit: number) => {
    const next = mode ^ bit;
    setModeValue(next);
    setOctal(next.toString(8).padStart(3, '0'));
    setSetMode(true);
  };

  const typeOctal = (text: string) => {
    setOctal(text);
    if (/^[0-7]{3,4}$/.test(text)) setModeValue(parseInt(text, 8));
    setSetMode(true);
  };

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (setMode && !/^[0-7]{3,4}$/.test(octal)) { setError(t('sftp.attrsBadMode')); return; }
    if (!setMode && !owner.trim() && !group.trim()) { setError(t('sftp.attrsNothing')); return; }
    setBusy(true);
    setError('');
    const res = await changeAttrs(source, {
      paths: targets.map((x) => x.path),
      mode: setMode ? octal : undefined,
      owner: owner.trim() || undefined,
      group: group.trim() || undefined,
      recursive: folders && recursive,
    });
    setBusy(false);
    if (!res.success || !res.data) { setError(res.message || t('common.error')); return; }
    onDone(res.data);
  };

  const title = targets.length === 1 ? targets[0].entry.name : t('sftp.selected', { count: targets.length });

  return createPortal(
    <div className="modal-overlay term-settings-overlay" onClick={onClose}>
      <form ref={dialogRef} className="modal-content sftp-attrs" role="dialog" aria-modal="true" aria-labelledby={titleId}
        onClick={(e) => e.stopPropagation()} onSubmit={submit}
        onKeyDown={(e) => { if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); onClose(); } }}>
        <h3 id={titleId}>{t('sftp.attrsTitle')}</h3>
        <p className="text-secondary sftp-attrs-what">{title}</p>
        {error && <div role="alert" className="message">{error}</div>}

        <fieldset className="sftp-attrs-mode">
          <legend>
            <label>
              <input type="checkbox" checked={setMode} onChange={(e) => setSetMode(e.target.checked)} />{' '}
              {t('sftp.attrsChangeMode')}
            </label>
          </legend>
          {shared === undefined && <small className="text-secondary">{t('sftp.attrsMixed')}</small>}
          <table>
            <thead><tr><th />{WHAT.map(([what]) => <th key={what}>{t(`sftp.attrs.${what}`)}</th>)}</tr></thead>
            <tbody>
              {WHO.map((who, row) => (
                <tr key={who}>
                  <th scope="row">{t(`sftp.attrs.${who}`)}</th>
                  {WHAT.map(([what, value]) => {
                    const bit = value << ((2 - row) * 3);
                    return (
                      <td key={what}>
                        <input type="checkbox" checked={(mode & bit) !== 0} onChange={() => toggle(bit)}
                          aria-label={`${t(`sftp.attrs.${who}`)} ${t(`sftp.attrs.${what}`)}`} />
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
          <div className="sftp-attrs-octal">
            <input className="filter-input" value={octal} maxLength={4} inputMode="numeric"
              aria-label={t('sftp.attrsOctal')} onChange={(e) => typeOctal(e.target.value.trim())} />
            <code>{modeString(mode)}</code>
          </div>
        </fieldset>

        {caps.owner ? (
          <div className="sftp-attrs-owners">
            <label>{t('sftp.attrs.ownerName')}
              <input className="filter-input" value={owner} onChange={(e) => setOwner(e.target.value)}
                list={`${listId}-users`} placeholder={currentOwner ?? t('sftp.attrsKeepMixed')} />
            </label>
            <label>{t('sftp.attrs.groupName')}
              <input className="filter-input" value={group} onChange={(e) => setGroup(e.target.value)}
                list={`${listId}-groups`} placeholder={currentGroup ?? t('sftp.attrsKeepMixed')} />
            </label>
            <datalist id={`${listId}-users`}>{caps.users.map((u) => <option key={u} value={u} />)}</datalist>
            <datalist id={`${listId}-groups`}>{caps.groups.map((g) => <option key={g} value={g} />)}</datalist>
            <small className="text-secondary">{t(caps.users.length ? 'sftp.attrsOwnerHint' : 'sftp.attrsOwnerNumbers')}</small>
          </div>
        ) : (
          <p className="text-secondary">{t('sftp.attrsNoOwner')}</p>
        )}

        {folders && (
          <label className="sftp-attrs-recursive">
            <input type="checkbox" checked={recursive} onChange={(e) => setRecursive(e.target.checked)} />{' '}
            {t('sftp.attrsRecursive')}
            <small className="text-secondary">{t('sftp.attrsRecursiveHint')}</small>
          </label>
        )}

        <div className="btn-group">
          <button type="submit" className="btn btn-primary" disabled={busy}>{busy ? t('common.working') : t('sftp.attrsApply')}</button>
          <button type="button" className="btn btn-secondary" onClick={onClose}>{t('common.cancel')}</button>
        </div>
      </form>
    </div>,
    document.body,
  );
}
