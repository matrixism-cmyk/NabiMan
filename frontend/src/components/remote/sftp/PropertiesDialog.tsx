import React, { useEffect, useRef, useState } from 'react';
import { useT } from '../../../shared/i18n';
import Modal from '../../../shared/ui/Modal';
import { formatSize, modeString, SftpSource } from './sftpApi';
import { checksum, usage, UsageCount, UsageInside, UsageItem, UsageLine } from './sftpUsage';
import { formatDateTime, formatNumber } from '../../../shared/i18n/format';

interface Props {
  source: SftpSource;
  /** Full paths of what to describe: the chosen rows, or the open folder. */
  paths: string[];
  /** Opens the permissions dialog for these, where it can be used. */
  onAttrs?: (items: UsageItem[]) => void;
  onClose: () => void;
}

type Phase = 'counting' | 'done' | 'stopped' | 'failed';

const n = (v: number) => formatNumber(v);
const exact = (bytes: number) => `${formatSize(bytes)} (${n(bytes)} B)`;
const fullTime = (mtime: number | null) => (mtime ? formatDateTime(mtime * 1000) : '-');

/**
 * Properties of one entry or many: what each is, and — for folders — how many
 * folders and files are inside all the way down and how big they are. The
 * count arrives as it goes (a large tree takes a while), can be stopped, and
 * says when a limit or an unreadable folder made it fall short.
 */
export default function PropertiesDialog({ source, paths, onAttrs, onClose }: Props) {
  const { t } = useT();
  const [items, setItems] = useState<UsageItem[] | null>(null);
  const [inside, setInside] = useState<UsageInside | null>(null);
  const [count, setCount] = useState<UsageCount | null>(null);
  const [last, setLast] = useState<UsageLine | null>(null);
  const [phase, setPhase] = useState<Phase>('counting');
  const [error, setError] = useState('');
  const job = useRef<AbortController | null>(null);

  useEffect(() => {
    const ctl = new AbortController();
    job.current = ctl;
    usage(source, paths, (line) => {
      if (line.items) { setItems(line.items); setInside(line.inside ?? null); }
      if (line.count) setCount(line.count);
      if (line.done) { setLast(line); setPhase('done'); }
    }, ctl.signal)
      .then((why) => { if (why) { setError(why); setPhase('failed'); } })
      .catch((e) => { if (!ctl.signal.aborted) { setError(e instanceof Error ? e.message : String(e)); setPhase('failed'); } });
    return () => ctl.abort();
  }, [source, paths]);

  const stop = () => { job.current?.abort(); setPhase('stopped'); };
  const single = items?.length === 1 ? items[0] : null;
  const folders = (items || []).filter((i) => i.kind === 'dir');
  const counting = phase === 'counting';
  const kindOf = (i: UsageItem) => t(`sftp.props.kind.${i.kind}`);

  const summary = (c: UsageCount) => [
    t('sftp.props.nDirs', { count: n(c.dirs) }), t('sftp.props.nFiles', { count: n(c.files) }),
    ...(c.links ? [t('sftp.props.nLinks', { count: n(c.links) })] : []),
    ...(c.other ? [t('sftp.props.nOther', { count: n(c.other) })] : []),
  ].join(' · ');

  return (
    <Modal title={t('sftp.props.title')} className="sftp-props" onClose={onClose}>
      {error && <div role="alert" className="message">{error}</div>}
      {!items && !error && <p className="text-secondary">{t('sftp.props.loading')}</p>}
      {items && (
        <dl className="sftp-props-list">
          {single ? (
            <>
              <dt>{t('sftp.props.name')}</dt><dd><strong>{single.name || '/'}</strong></dd>
              <dt>{t('sftp.props.where')}</dt><dd><code>{single.path}</code></dd>
              <dt>{t('sftp.props.kind')}</dt>
              <dd>{kindOf(single)}{single.target !== undefined && <> → <code>{single.target}</code></>}</dd>
              {single.kind === 'file' && <><dt>{t('sftp.props.size')}</dt><dd>{exact(single.size)}</dd></>}
              <dt>{t('sftp.props.mtime')}</dt><dd>{fullTime(single.mtime)}</dd>
            </>
          ) : (
            <>
              <dt>{t('sftp.props.chosen')}</dt>
              <dd>{t('sftp.selected', { count: items.length })} — {summary(tally(items))}</dd>
            </>
          )}
          {single && single.mode !== null && (
            <>
              <dt>{t('sftp.props.mode')}</dt>
              <dd>
                <code>{modeString(single.mode)}</code> ({single.mode.toString(8).padStart(3, '0')})
                {onAttrs && <button type="button" className="sftp-link" onClick={() => onAttrs(items)}>🔐 {t('sftp.props.change')}</button>}
              </dd>
            </>
          )}
          {!single && onAttrs && (
            <><dt>{t('sftp.props.mode')}</dt><dd><button type="button" className="sftp-link" onClick={() => onAttrs(items)}>🔐 {t('sftp.props.change')}</button></dd></>
          )}
          {single && (single.owner || single.uid !== undefined) && (
            <><dt>{t('sftp.props.owner')}</dt><dd>{single.owner ?? single.uid} : {single.group ?? single.gid ?? '-'}</dd></>
          )}
          {inside && (
            <>
              <dt>{t('sftp.props.inside')}</dt>
              <dd>
                {summary({ ...inside, bytes: 0, unreadable: 0 })}
                {inside.hidden > 0 && <span className="text-secondary"> ({t('sftp.props.nHidden', { count: n(inside.hidden) })})</span>}
              </dd>
            </>
          )}
          {(folders.length > 0 || (items.length > 1)) && count && (
            <>
              <dt>{t(single ? 'sftp.props.all' : 'sftp.props.allChosen')}</dt>
              <dd aria-live="polite">
                {summary(count)}{(last?.capped || phase === 'stopped') && ` ${t('sftp.props.atLeast')}`}<br />
                {t('sftp.props.bytes')}: {exact(count.bytes)}
                {counting && <span className="sftp-props-busy"> · {t('sftp.props.counting', { count: n(count.dirs + count.files + count.links + count.other) })}</span>}
              </dd>
            </>
          )}
        </dl>
      )}
      {items && folders.length > 0 && <Notes phase={phase} last={last} count={count} paths={paths} />}
      {single?.kind === 'file' && <Checksum source={source} path={single.path} />}
      <div className="btn-group">
        {counting && folders.length > 0 && <button type="button" className="btn btn-secondary" onClick={stop}>{t('sftp.props.stop')}</button>}
        <button type="button" className="btn btn-primary" onClick={onClose}>{t('common.close')}</button>
      </div>
    </Modal>
  );
}

/** The chosen entries themselves, by kind. */
function tally(items: UsageItem[]): UsageCount {
  const c: UsageCount = { dirs: 0, files: 0, links: 0, other: 0, bytes: 0, unreadable: 0 };
  for (const i of items) {
    if (i.kind === 'dir') c.dirs += 1; else if (i.kind === 'file') c.files += 1; else if (i.kind === 'link') c.links += 1; else c.other += 1;
  }
  return c;
}

/** Why the count may fall short of the truth. */
function Notes({ phase, last, count, paths }: { phase: Phase; last: UsageLine | null; count: UsageCount | null; paths: string[] }) {
  const { t } = useT();
  const notes: string[] = [];
  if (phase === 'counting' && count && count.dirs > 500) notes.push(t('sftp.props.slow'));
  if (phase === 'stopped') notes.push(t('sftp.props.stopped'));
  if (last?.capped) notes.push(t(`sftp.props.capped.${last.capped}`));
  if (count?.unreadable) notes.push(t('sftp.props.unreadable', { count: formatNumber(count.unreadable) }));
  if (paths.includes('/')) notes.push(t('sftp.props.system'));
  return notes.length ? <ul className="sftp-props-notes text-secondary">{notes.map((x) => <li key={x}>{x}</li>)}</ul> : null;
}

/** A file's SHA-256, worked out only when asked: it reads the whole file. */
function Checksum({ source, path }: { source: SftpSource; path: string }) {
  const { t } = useT();
  const [state, setState] = useState<{ read: number; size: number; sha?: string; error?: string } | null>(null);
  const job = useRef<AbortController | null>(null);
  useEffect(() => () => job.current?.abort(), []);
  const start = () => {
    const ctl = new AbortController();
    job.current = ctl;
    setState({ read: 0, size: 0 });
    checksum(source, path, (l) => setState({ read: l.read ?? 0, size: l.size ?? 0, sha: l.sha256, error: l.error }), ctl.signal)
      .then((why) => { if (why) setState((s) => ({ read: s?.read ?? 0, size: s?.size ?? 0, error: why })); })
      .catch(() => { /* closed */ });
  };
  return (
    <div className="sftp-props-sum">
      <span>SHA-256</span>
      {!state && <button type="button" className="btn btn-secondary btn-sm" onClick={start}>{t('sftp.props.sumStart')}</button>}
      {state && !state.sha && !state.error && (
        <span className="text-secondary">{t('sftp.props.sumRunning', { pct: state.size ? Math.floor((state.read / state.size) * 100) : 0 })}</span>
      )}
      {state?.sha && (
        <>
          <code className="sftp-props-hash">{state.sha}</code>
          <button type="button" className="sftp-link" onClick={() => navigator.clipboard?.writeText(state.sha!).catch(() => undefined /* clipboard refused: the checksum stays on screen to select */)}>{t('common.copy')}</button>
        </>
      )}
      {state?.error && <span role="alert" className="message">{state.error}</span>}
    </div>
  );
}
