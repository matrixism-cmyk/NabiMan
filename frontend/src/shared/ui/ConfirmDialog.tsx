import React, { useEffect, useRef, useState } from 'react';
import { useT } from '../i18n';
import Modal from './Modal';
import './confirm-dialog.css';

export interface ConfirmOptions {
  message: string;
  title?: string;
  /** Names of what is affected, shown as a list (the first few). */
  items?: string[];
  confirmLabel?: string;
  /** Deleting, ending, revoking: the confirm button is red, and focus starts on cancel. */
  danger?: boolean;
}

interface Pending extends ConfirmOptions {
  resolve: (ok: boolean) => void;
  /** Each question is its own modal, so focus starts afresh. */
  id?: number;
}

type Listener = (request: Pending) => void;
let listener: Listener | null = null;
let asked = 0;

/**
 * Asks before doing something, in the app's own modal instead of the
 * browser's `confirm()`. Resolves true when confirmed. Where no host is
 * mounted (a test rendering one component), it falls back to `confirm()`.
 */
export function confirmDialog(options: ConfirmOptions | string): Promise<boolean> {
  const opts = typeof options === 'string' ? { message: options } : options;
  if (!listener) return Promise.resolve(window.confirm(opts.message));
  const show = listener;
  return new Promise((resolve) => show({ ...opts, resolve }));
}

const SHOWN_ITEMS = 8;

/** Mounted once at the root; shows each question as it is asked, in turn. */
export function ConfirmHost() {
  const { t } = useT();
  const [queue, setQueue] = useState<Pending[]>([]);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const okRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    const mine: Listener = (request) => setQueue((q) => [...q, { ...request, id: ++asked }]);
    listener = mine;
    return () => { if (listener === mine) listener = null; };
  }, []);

  const current = queue[0];
  if (!current) return null;

  const answer = (ok: boolean) => {
    current.resolve(ok);
    setQueue((q) => q.slice(1));
  };
  const items = current.items || [];

  return (
    <Modal key={current.id} role="alertdialog" level="top" className="confirm-dialog" onClose={() => answer(false)}
      initialFocus={current.danger ? cancelRef : okRef}
      title={<span className={current.danger ? 'is-danger' : ''}>{current.title || t(current.danger ? 'confirm.dangerTitle' : 'confirm.title')}</span>}>
      <p className="confirm-message">{current.message}</p>
      {items.length > 0 && (
        <ul className="confirm-items">
          {items.slice(0, SHOWN_ITEMS).map((item, i) => <li key={`${i}:${item}`}>{item}</li>)}
          {items.length > SHOWN_ITEMS && (
            <li className="confirm-more">{t('confirm.more', { count: items.length - SHOWN_ITEMS })}</li>
          )}
        </ul>
      )}
      <div className="modal-actions confirm-actions">
        <button ref={cancelRef} type="button" className="btn btn-secondary" onClick={() => answer(false)}>
          {t('common.cancel')}
        </button>
        <button ref={okRef} type="button" className={`btn ${current.danger ? 'btn-danger' : 'btn-primary'}`} onClick={() => answer(true)}>
          {current.confirmLabel || t('common.confirm')}
        </button>
      </div>
    </Modal>
  );
}
