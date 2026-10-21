import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import TerminalView, { TerminalViewHandle } from './TerminalView';
import { DEFAULT_TERMINAL_SETTINGS } from './settings';
import { issueShareTicket, readShareInfo, ShareAdmissionError, ShareFilesGrant, ShareInfo } from './shareAdmission';
import TerminalWithFiles from '../remote/TerminalWithFiles';
import SftpPanel from '../remote/sftp/SftpPanel';
import DictationButton from './dictation/DictationButton';
import { openShareDictation } from './dictation/sockets';
import { shareSource } from '../remote/sftp/sftpApi';
import { useT } from '../../shared/i18n';
import './terminal-windows.css';

/**
 * What someone sees when they open a share link: no NabiMan account, no
 * navigation — the shared terminal and nothing else.
 */
export default function SharePage({ token }: { token: string }) {
  return <SharePageContent key={token} token={token} />;
}
function SharePageContent({ token }: { token: string }) {
  const { t } = useT();
  const viewRef = useRef<TerminalViewHandle>(null);
  const [info, setInfo] = useState<ShareInfo | null>(null);
  const [error, setError] = useState('');
  const [password, setPassword] = useState('');
  const [authorized, setAuthorized] = useState(false);
  // Needed only for fresh admission after a network reconnect. Never put this
  // password in a URL, browser storage, a cookie or the terminal's props.
  const passwordRef = useRef('');
  const prepared = useRef<{ ticket: string; expires: number; query: string } | null>(null);
  const submitController = useRef<AbortController | null>(null);
  const [checking, setChecking] = useState(false);
  // The file panel's key comes with each admission and lives in memory only.
  const [files, setFiles] = useState<ShareFilesGrant | null>(null);
  const filesSource = useMemo(
    () => (files ? shareSource(token, files.key, files.mode === 'write') : null),
    [token, files],
  );

  const load = useCallback(async (signal: AbortSignal) => {
    try {
      const data = await readShareInfo(token, signal);
      if (signal.aborted) return;
      if (!data.needs_password && (data.files === 'read' || data.files === 'write')) {
        // Admit now, as a password would, so the file panel is there before
        // the terminal mounts instead of wrapping it (and reconnecting) later.
        const admitted = await issueShareTicket(token, '', '', signal);
        if (signal.aborted) return;
        prepared.current = { ticket: admitted.ticket, query: '', expires: performance.now() + admitted.expires_in_seconds * 1000 };
        setFiles(admitted.files || null);
      }
      setInfo(data); setError('');
      if (!data.needs_password) setAuthorized(true);
    } catch (cause) {
      if (signal.aborted) return;
      setInfo(null);
      setError(t(cause instanceof ShareAdmissionError ? cause.translationKey : 'share.authUnavailable'));
    }
  }, [token, t]);

  useEffect(() => {
    const controller = new AbortController(); void load(controller.signal);
    return () => controller.abort();
  }, [load]);
  useEffect(() => () => { submitController.current?.abort(); passwordRef.current = ''; prepared.current = null; }, []);

  const acquireTicket = useCallback(async (query: string, signal?: AbortSignal) => {
    if (signal?.aborted) throw new DOMException('Aborted', 'AbortError');
    // A ticket prepared for the terminal is only taken by the terminal.
    const ready = prepared.current;
    if (ready && ready.query === query) prepared.current = null;
    if (ready && ready.query === query && ready.expires > performance.now()) return ready.ticket;
    try {
      const admitted = await issueShareTicket(token, passwordRef.current, query, signal);
      if (admitted.files) setFiles(admitted.files);
      return admitted.ticket;
    } catch (cause) {
      if (signal?.aborted) throw cause;
      throw new Error(t(cause instanceof ShareAdmissionError ? cause.translationKey : 'share.authUnavailable'), { cause });
    }
  }, [token, t]);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    submitController.current?.abort();
    const controller = new AbortController(); submitController.current = controller;
    setChecking(true);
    try {
      const data = await issueShareTicket(token, password, '', controller.signal);
      if (controller.signal.aborted) return;
      passwordRef.current = password;
      prepared.current = { ticket: data.ticket, query: '', expires: performance.now() + data.expires_in_seconds * 1000 };
      setFiles(data.files || null);
      setPassword(''); setAuthorized(true); setError('');
    } catch (cause) {
      if (!controller.signal.aborted) setError(t(cause instanceof ShareAdmissionError ? cause.translationKey : 'share.authUnavailable'));
    } finally { if (!controller.signal.aborted) setChecking(false); }
  };

  if (error && !info) {
    return (
      <div className="share-page share-page-center">
        <div className="share-gate">
          <h1>{t('share.invalidTitle')}</h1>
          <p className="text-secondary">{error}</p>
        </div>
      </div>
    );
  }

  if (!info) {
    return <div className="share-page share-page-center"><p className="text-secondary">{t('common.loading')}</p></div>;
  }

  if (!authorized) {
    return (
      <div className="share-page share-page-center">
        <form className="share-gate" onSubmit={submit}>
          <h1>{t('share.gateTitle')}</h1>
          <p className="text-secondary">{info.label}</p>
          {error && <div className="login-error">{error}</div>}
          <input
            type="password" value={password} autoFocus autoComplete="current-password"
            placeholder={t('share.passwordPrompt')}
            onChange={(e) => setPassword(e.target.value)}
          />
          <button className="btn btn-primary" type="submit" disabled={checking}>
            {checking ? t('common.working') : t('share.enter')}
          </button>
        </form>
      </div>
    );
  }

  if (info.kind === 'files') {
    // A server's files alone: the file panel is the whole page.
    return (
      <div className="share-page">
        <header className="share-head">
          <strong>{t('share.filesHead')}</strong>
          <code>{info.label}</code>
          <span className="status-badge">{t(info.files === 'write' ? 'share.filesTagWrite' : 'share.filesTagRead')}</span>
          {!info.alive && <span className="status-badge down">{t('share.filesEnded')}</span>}
        </header>
        <div className="share-body share-body-files">
          {filesSource
            ? <SftpPanel source={filesSource} serverName={info.label} />
            : <p className="text-secondary">{t('share.filesEnded')}</p>}
        </div>
      </div>
    );
  }

  return (
    <div className="share-page">
      <header className="share-head">
        <strong>{t('share.headTitle')}</strong>
        <code>{info.label}</code>
        {info.read_only && <span className="status-badge">{t('share.readOnly')}</span>}
        {!info.alive && <span className="status-badge down">{t('share.ended')}</span>}
        <span className="share-head-spacer" />
        {info.dictation && !info.read_only && (
          <DictationButton open={() => openShareDictation(token, (query) => acquireTicket(query))} endpointMs={info.dictation_pause_ms} enterOnPause={info.dictation_auto_enter}
            onText={(text) => viewRef.current?.insertText(text)} onKeys={(keys) => viewRef.current?.sendKeys(keys)} />
        )}
        <button
          className="btn btn-secondary btn-sm"
          title={t('share.fitWindowHint')}
          onClick={() => viewRef.current?.resetZoom()}
        >
          {t('share.fitWindow')}
        </button>
        <button className="btn btn-secondary btn-sm" onClick={() => viewRef.current?.reconnect()}>
          {t('terminal.reconnect')}
        </button>
      </header>
      <div className="share-body">
        <TerminalWithFiles target={null} name={info.label} source={filesSource}
          onInsert={(text) => viewRef.current?.insertText(text)}>
          <TerminalView
            ref={viewRef}
            target={{ kind: 'local' }}
            storageKey={`share_${token}`}
            settings={DEFAULT_TERMINAL_SETTINGS}
            share={{ token, acquireTicket }}
            grid={{ cols: info.cols, rows: info.rows }}
            active
          />
        </TerminalWithFiles>
      </div>
    </div>
  );
}
