import React, { useEffect, useId, useMemo, useState } from 'react';
import Modal from '../../shared/ui/Modal';
import { apiRequest } from '../../shared/api/useApi';
import { useT } from '../../shared/i18n';
import type { ClaudeStart } from '../terminal/terminalTypes';
import '../terminal/claude-address.css';

interface Account { name: string; home: string }
interface Place { user: string; dir: string; last_used: number | null }
interface Options { users: Account[]; default_user: string | null; dirs: Place[] }

interface Props {
  onClose: () => void;
  onStart: (start: ClaudeStart) => void;
}

const lastPart = (dir: string) => dir.replace(/\/+$/, '').split('/').pop() || dir;

/**
 * Asks how a new local session should start Claude Code: as which of this
 * machine's users, in which directory, with which options.
 */
export default function ClaudeStartDialog({ onClose, onStart }: Props) {
  const { t } = useT();
  const listId = useId();
  const [options, setOptions] = useState<Options | null>(null);
  const [error, setError] = useState('');
  const [user, setUser] = useState('');
  const [dir, setDir] = useState('');
  const [title, setTitle] = useState('');
  const [titleTouched, setTitleTouched] = useState(false);
  const [skipPermissions, setSkip] = useState(true);
  const [resume, setResume] = useState(true);

  useEffect(() => {
    let live = true;
    Promise.resolve(apiRequest<Options>('/api/terminal/start-options'))
      .then((res) => {
        if (!live) return;
        if (!res?.success || !res.data) { setError(t('claudeStart.loadFailed')); return; }
        setOptions(res.data);
        const first = res.data.default_user || res.data.users.find(u => u.name !== 'root')?.name || res.data.users[0]?.name || '';
        setUser(first);
        const place = res.data.dirs.find(p => p.user === first);
        if (place) setDir(place.dir);
      })
      .catch(() => { if (live) setError(t('claudeStart.loadFailed')); });
    return () => { live = false; };
  }, [t]);

  const places = useMemo(() => (options?.dirs || []).filter(p => p.user === user), [options, user]);
  const name = titleTouched ? title : (dir ? lastPart(dir) : '');
  const flags = `${skipPermissions ? ' --dangerously-skip-permissions' : ''}${resume ? ' -c' : ''}`;
  const preview = `${user && user !== 'root' ? `su ${user} → ` : ''}cd ${dir || '…'} && claude${flags}`;

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    const path = dir.trim();
    if (!path.startsWith('/')) { setError(t('claudeStart.needDir')); return; }
    onStart({ user, dir: path, skipPermissions, resume, title: name.trim() || undefined });
  };

  return (
    <Modal onClose={onClose} title={t('claudeStart.title')} className="claude-start">
      <form className="claude-start-form" onSubmit={submit}>
        {error && <p role="alert" className="message">{error}</p>}
        <label>
          <span>{t('claudeStart.user')}</span>
          <select className="filter-input" value={user} onChange={e => {
            setUser(e.target.value);
            const place = options?.dirs.find(p => p.user === e.target.value);
            if (place) setDir(place.dir);
          }} disabled={!options}>
            {options?.users.map(u => <option key={u.name} value={u.name}>{u.name} ({u.home})</option>)}
          </select>
        </label>
        <label>
          <span>{t('claudeStart.dir')}</span>
          <input className="filter-input" list={listId} value={dir} onChange={e => setDir(e.target.value)}
            placeholder="/srv/project" autoComplete="off" spellCheck={false} required />
          <datalist id={listId}>
            {places.map(p => <option key={p.dir} value={p.dir} />)}
          </datalist>
          <small className="text-secondary">{t('claudeStart.dirHint')}</small>
        </label>
        <label>
          <span>{t('claudeStart.name')}</span>
          <input className="filter-input" value={name} maxLength={64} onChange={e => { setTitleTouched(true); setTitle(e.target.value); }} />
        </label>
        <label className="claude-start-check">
          <input type="checkbox" checked={skipPermissions} onChange={e => setSkip(e.target.checked)} />
          <span>{t('claudeStart.skip')}</span>
        </label>
        <label className="claude-start-check">
          <input type="checkbox" checked={resume} onChange={e => setResume(e.target.checked)} />
          <span>{t('claudeStart.resume')}</span>
        </label>
        <p className="claude-start-preview" aria-label={t('claudeStart.command')}><code>{preview}</code></p>
        <div className="btn-group">
          <button type="submit" className="btn btn-primary" disabled={!options || !user}>{t('claudeStart.start')}</button>
          <button type="button" className="btn btn-secondary" onClick={onClose}>{t('claudeStart.cancel')}</button>
        </div>
      </form>
    </Modal>
  );
}
