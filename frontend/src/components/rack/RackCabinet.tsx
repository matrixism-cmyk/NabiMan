import React, { useMemo } from 'react';
import { useT } from '../../i18n';
import type { Rack, RackDevice, RackNode, RackStatus } from './rackTypes';
import { arrange, slotNumbers } from './rackGeometry';
import { lookOf } from './rackFormat';
import RackDeviceFace from './RackDeviceFace';

interface Props {
  rack: Rack;
  nodes: Record<string, RackNode>;
  status: RackStatus | null;
  onOpen: (device: RackDevice) => void;
}

/** One cabinet: slot numbers down the side (U1 at the bottom), devices to scale, and under it the devices nobody has placed yet. */
export default function RackCabinet({ rack, nodes, status, onOpen }: Props) {
  const { t } = useT();
  const { placed, unplaced } = useMemo(() => arrange(rack.devices, rack.units), [rack]);
  const numbers = useMemo(() => slotNumbers(rack.units), [rack.units]);
  const face = (device: RackDevice, units: number | null) => (
    <RackDeviceFace key={device.id} device={device} look={lookOf(device, nodes)} rackName={rack.name} units={units} status={status} onOpen={onOpen} />
  );
  return (
    <section className="rack-cabinet" aria-label={t('rack.rack.aria', { name: rack.name, units: rack.units })} data-rack={rack.id}>
      <header className="rack-cabinet-head">
        <strong>{rack.name}</strong>
        {rack.role && <span>{rack.role}</span>}
      </header>
      <div className="rack-body" style={{ '--units': rack.units } as React.CSSProperties}>
        <ol className="rack-rail" aria-hidden="true">
          {numbers.map((n) => <li key={n}>{n}</li>)}
        </ol>
        <div className="rack-slots">
          {placed.map((device) => face(device, rack.units))}
        </div>
      </div>
      {unplaced.length > 0 && (
        <div className="rack-unplaced">
          <h4>{t('rack.unplaced')}</h4>
          {unplaced.map((device) => face(device, null))}
        </div>
      )}
    </section>
  );
}
