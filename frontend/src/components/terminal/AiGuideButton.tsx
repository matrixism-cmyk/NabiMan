import React, { RefObject, useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useT } from '../../shared/i18n';
import RailContextMenu, { RailMenuItem } from '../remote/RailContextMenu';
import type { TerminalViewHandle } from './terminalTypes';
import { copyText } from './ClaudeAddress';
import { bracketedPaste, fetchGuide, useGuideAllowed } from './aiGuide';
import AiGuidePreview, { guideFailure } from './AiGuidePreview';
import './ai-guide.css';

interface Props {
  /** The attached session (its pty id); nothing is shown without one. */
  sessionId: string;
  handle: RefObject<TerminalViewHandle | null>;
  /** Pasting needs a live connection. */
  connected: boolean;
  /** `head`: the workspace header; `bar`: a window's title bar; `icon`: a phone's header. */
  variant: 'head' | 'bar' | 'icon';
}

type Note = { kind: 'ok' | 'error' | 'busy'; text: string };

/** How long "Enter를 눌러 보내세요" stays beside the button. */
const TOAST_MS = 6000;

/**
 * "AI 안내": the server's orchestration guide for the AI working in this
 * session — copied, pasted straight into the session (as one bracketed
 * paste, Enter left to the operator), or read first. Hidden from remote-only
 * members, whom the server does not give the guide.
 */
export default function AiGuideButton({ sessionId, handle, connected, variant }: Props) {
  const { t } = useT();
  const allowed = useGuideAllowed();
  const buttonRef = useRef<HTMLButtonElement>(null);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [note, setNote] = useState<Note | null>(null);
  const [toast, setToast] = useState<{ top: number; right: number } | null>(null);
  const [preview, setPreview] = useState(false);
  // A click on the button while the menu is open closes it (on mousedown,
  // outside the menu); the click that follows must not open it again.
  const wasOpen = useRef(false);
  const busy = useRef(false);

  const close = useCallback(() => { setMenu(null); setNote(null); }, []);

  useEffect(() => {
    if (!toast) return undefined;
    const timer = window.setTimeout(() => setToast(null), TOAST_MS);
    return () => window.clearTimeout(timer);
  }, [toast]);

  if (!allowed || !sessionId) return null;

  const open = () => {
    const r = buttonRef.current?.getBoundingClientRect();
    if (!r) return;
    setNote(null);
    setToast(null);
    setMenu({ x: r.left, y: r.bottom + 4 });
  };

  const failed = (e: unknown) => {
    const why = guideFailure(e);
    setNote({ kind: 'error', text: why ? `${t('aiGuide.loadFailed')} (${why})` : t('aiGuide.loadFailed') });
  };

  /** Fetches the guide (or reuses the last minute's) and does `then` with it. */
  const withGuide = async (then: (text: string) => Promise<void> | void) => {
    if (busy.current) return;
    busy.current = true;
    setNote({ kind: 'busy', text: t('aiGuide.loading') });
    try {
      const guide = await fetchGuide(sessionId);
      await then(guide.text);
    } catch (e) {
      failed(e);
    } finally {
      busy.current = false;
    }
  };

  const copy = () => withGuide(async (text) => {
    const ok = await copyText(text);
    setNote(ok ? { kind: 'ok', text: t('aiGuide.copied') } : { kind: 'error', text: t('aiGuide.copyFailed') });
  });

  const paste = () => withGuide((text) => {
    if (!handle.current?.paste(bracketedPaste(text))) {
      setNote({ kind: 'error', text: t('aiGuide.notConnected') });
      return;
    }
    // The menu goes and the terminal has the keys: the next Enter sends it.
    const r = buttonRef.current?.getBoundingClientRect();
    close();
    if (r) setToast({ top: r.bottom + 6, right: Math.max(8, window.innerWidth - r.right) });
  });

  const items: RailMenuItem[] = [
    { key: 'copy', icon: '⧉', label: t('aiGuide.copy'), keepOpen: true, onSelect: copy },
    { key: 'paste', icon: '⇥', label: t('aiGuide.paste'), keepOpen: true, disabled: !connected, onSelect: paste },
    {
      key: 'preview', icon: '☰', label: t('aiGuide.preview'),
      // The dialog gives focus back to the button when it closes.
      onSelect: () => { buttonRef.current?.focus(); setPreview(true); },
    },
  ];

  const footer = (
    <>
      {note && (
        <p className={`ai-guide-note is-${note.kind}`} role={note.kind === 'error' ? 'alert' : 'status'}>{note.text}</p>
      )}
      <p className="ai-guide-hint">{t('aiGuide.hint')}</p>
    </>
  );

  const label = variant === 'head' ? t('aiGuide.button') : t('aiGuide.short');
  const className = variant === 'head' ? 'btn btn-secondary btn-sm ai-guide-btn'
    : variant === 'icon' ? 'rw-icon-btn ai-guide-btn' : 'ai-guide-btn ai-guide-btn-bar';
  const stop = (e: React.SyntheticEvent) => e.stopPropagation();

  // What opens from here is portalled, yet React still bubbles its events to
  // the title bar: a press on the menu or the dialog must not drag the window.
  return (
    <span className="ai-guide" onPointerDown={stop} onDoubleClick={stop}>
      <button ref={buttonRef} type="button" className={className} data-testid="ai-guide-button"
        aria-label={t('aiGuide.button')} title={t('aiGuide.buttonHint')}
        aria-haspopup="menu" aria-expanded={!!menu}
        onMouseDown={() => { wasOpen.current = !!menu; }}
        onClick={(e) => {
          e.stopPropagation();
          if (wasOpen.current) { wasOpen.current = false; return; }
          open();
        }}
      >{label}</button>
      {menu && (
        <RailContextMenu x={menu.x} y={menu.y} title={t('aiGuide.button')} items={items}
          onClose={close} opener={buttonRef.current} footer={footer} className="ai-guide-menu" />
      )}
      {toast && createPortal(
        <p className="ai-guide-toast" role="status" style={{ top: toast.top, right: toast.right }}>{t('aiGuide.pasted')}</p>,
        document.body,
      )}
      {preview && <AiGuidePreview sessionId={sessionId} onClose={() => setPreview(false)} />}
    </span>
  );
}
