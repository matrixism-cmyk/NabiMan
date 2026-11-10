import React, { lazy, Suspense } from 'react';
import { useT } from '../../shared/i18n';

/**
 * The VM consoles bring noVNC and Guacamole with them; they are fetched the
 * first time a console opens instead of with every page.
 */
export const VncConsoleView = lazy(() => import('./VncConsoleView'));
export const RdpConsoleView = lazy(() => import('./RdpConsoleView'));

export function ConsoleSuspense({ children }: { children: React.ReactNode }) {
  const { t } = useT();
  return (
    <Suspense fallback={<div className="rw-view rw-console-missing" role="status">{t('remote.console.loading')}</div>}>
      {children}
    </Suspense>
  );
}
