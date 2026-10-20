import React, { useEffect, useRef, useState } from 'react';
import { useT } from '../i18n';

/**
 * The header's controls. A wide screen shows them in a row; a phone folds
 * them under one "⋯" button so the header takes a single line, opening them
 * as a dropdown until something is picked or the page is touched elsewhere.
 */
export default function HeaderMenu({ children }: { children: React.ReactNode }) {
  const { t } = useT();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return undefined;
    const away = (e: PointerEvent) => { if (!ref.current?.contains(e.target as Node)) setOpen(false); };
    const key = (e: KeyboardEvent) => { if (e.key === 'Escape') setOpen(false); };
    document.addEventListener('pointerdown', away);
    document.addEventListener('keydown', key);
    return () => { document.removeEventListener('pointerdown', away); document.removeEventListener('keydown', key); };
  }, [open]);
  return (
    <div className="header-menu" ref={ref}>
      <button type="button" className="btn btn-secondary header-more" aria-expanded={open}
        aria-label={t('app.menu')} onClick={() => setOpen((o) => !o)}>⋯</button>
      <div className={`header-actions ${open ? 'is-open' : ''}`}
        onClick={(e) => { if ((e.target as HTMLElement).closest('button')) setOpen(false); }}>
        {children}
      </div>
    </div>
  );
}
