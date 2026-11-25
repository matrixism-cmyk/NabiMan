import React from 'react';
import { useT } from '../../shared/i18n';
import { SNAP_LAYOUTS, Zone, zoneRect } from './snapLayouts';

/**
 * Shown while a window is dragged to the top of the page: the arrangements
 * to drop it into, and — once the pointer is over one — where on the page the
 * window would land. The drag keeps the pointer, so nothing here handles
 * events; the frame asks which zone is under the pointer (data-snap).
 */
export default function SnapPicker({ over }: { over: Zone | null }) {
  const { t } = useT();
  const land = over ? zoneRect(over, window.innerWidth, window.innerHeight) : null;
  return (
    <>
      {land && <div className="term-snap-land" style={{ left: land.x, top: land.y, width: land.w, height: land.h }} />}
      <div className="term-snap-picker" role="group" aria-label={t('snap.title')}>
        {SNAP_LAYOUTS.map((zones, layout) => (
          <div className="term-snap-layout" key={layout}>
            {zones.map((zone, index) => (
              <div key={index} data-snap={`${layout}:${index}`} className={`term-snap-zone ${zone === over ? 'is-over' : ''}`}
                style={{ left: `${zone.x * 100}%`, top: `${zone.y * 100}%`, width: `${zone.w * 100}%`, height: `${zone.h * 100}%` }} />
            ))}
          </div>
        ))}
      </div>
    </>
  );
}
