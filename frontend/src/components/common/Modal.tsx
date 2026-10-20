import React, { useEffect, useId, useRef } from 'react';
import { createPortal } from 'react-dom';
import './modal.css';

/** Open modals, newest last: Escape belongs to the one on top. */
const stack: symbol[] = [];

const FOCUSABLE = 'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex]:not([tabindex="-1"])';

export interface ModalProps {
  /** Called on Escape, on a click outside, and by the caller's own buttons. */
  onClose: () => void;
  /** The heading; also names the dialog for screen readers. */
  title?: React.ReactNode;
  /** Extra class on the dialog box (its size and layout). */
  className?: string;
  /** `alertdialog` for a question that needs an answer. */
  role?: 'dialog' | 'alertdialog';
  /** What gets focus first; otherwise the first control, or the box itself. */
  initialFocus?: React.RefObject<HTMLElement | null>;
  /** Stack above other modals (a question asked from inside one). */
  level?: 'normal' | 'top';
  /** Keys the dialog handles itself (arrows in a viewer); return true if handled. */
  onKey?: (e: React.KeyboardEvent) => boolean | void;
  children: React.ReactNode;
}

/**
 * The frame every modal shares: rendered over the page, closed by Escape or
 * a click outside, with Tab kept inside and focus returned where it was when
 * it closes. Keys typed inside never reach what is behind it — a terminal, a
 * file list's shortcuts — even through React's portal bubbling.
 */
export default function Modal({ onClose, title, className = '', role = 'dialog', initialFocus, level = 'normal', onKey, children }: ModalProps) {
  const boxRef = useRef<HTMLDivElement>(null);
  const titleId = useId();
  const closeRef = useRef(onClose);
  closeRef.current = onClose;

  useEffect(() => {
    const me = Symbol('modal');
    stack.push(me);
    const previous = document.activeElement as HTMLElement | null;
    const box = boxRef.current;
    const first = initialFocus?.current || box?.querySelector<HTMLElement>(FOCUSABLE) || box;
    first?.focus();
    // Focus can fall out of the box (the button pressed was removed); Escape
    // still closes the modal on top, as long as no other modal has the key.
    const stray = (e: KeyboardEvent) => {
      const inModal = e.target instanceof Element && e.target.closest('.common-modal');
      if (e.key === 'Escape' && stack[stack.length - 1] === me && !inModal) { e.preventDefault(); closeRef.current(); }
    };
    document.addEventListener('keydown', stray);
    return () => {
      document.removeEventListener('keydown', stray);
      stack.splice(stack.indexOf(me), 1);
      if (previous && document.contains(previous)) previous.focus();
    };
    // Focus moves once, when the modal opens.
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const keyDown = (e: React.KeyboardEvent) => {
    e.stopPropagation();
    if (onKey?.(e)) { e.preventDefault(); return; }
    if (e.key === 'Escape') { e.preventDefault(); closeRef.current(); return; }
    if (e.key !== 'Tab' || !boxRef.current) return;
    const controls = Array.from(boxRef.current.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((el) => el.offsetParent !== null || el === document.activeElement);
    if (!controls.length) { e.preventDefault(); return; }
    const firstControl = controls[0], last = controls[controls.length - 1];
    const at = document.activeElement;
    if (e.shiftKey && (at === firstControl || at === boxRef.current)) { e.preventDefault(); last.focus(); }
    else if (!e.shiftKey && at === last) { e.preventDefault(); firstControl.focus(); }
  };

  return createPortal(
    <div className={`modal-overlay common-modal-overlay ${level === 'top' ? 'is-top' : ''}`}
      onMouseDown={(e) => { if (e.target === e.currentTarget) closeRef.current(); }}>
      <div ref={boxRef} className={`modal-content common-modal ${className}`} role={role} aria-modal="true"
        aria-labelledby={title ? titleId : undefined} tabIndex={-1}
        onKeyDown={keyDown} onClick={(e) => e.stopPropagation()}>
        {title && <h3 id={titleId} className="common-modal-title">{title}</h3>}
        {children}
      </div>
    </div>,
    document.body,
  );
}
