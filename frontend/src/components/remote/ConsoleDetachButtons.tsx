import React from 'react';
import { useT } from '../../shared/i18n';

/**
 * The two ways a console leaves the workspace's pane: "분리" into a window on
 * this page (on the bar below, arranged with the others) and "새창으로 분리"
 * into a browser window of its own (which can go to another monitor).
 */
export default function ConsoleDetachButtons({ onDetach, onPopOut }: { onDetach?: () => void; onPopOut: () => void }) {
  const { t } = useT();
  return (
    <>
      {onDetach && <button className="btn btn-secondary btn-sm" onClick={onDetach} title={t('remote.console.detachHint')}>{t('remote.console.detach')}</button>}
      <button className="btn btn-secondary btn-sm" onClick={onPopOut} title={t('remote.console.popOutHint')}>{t('remote.console.popOut')}</button>
    </>
  );
}
