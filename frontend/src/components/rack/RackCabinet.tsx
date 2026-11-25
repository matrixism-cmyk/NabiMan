import React, { useMemo } from 'react';
import { useT } from '../../i18n';
import type { Rack, RackDevice, RackNode, RackStatus } from './rackTypes';
import { arrange, deviceBox, slotLabel, slotNumbers } from './rackGeometry';
import { lookOf } from './rackFormat';
import RackDeviceFace from './RackDeviceFace';
import type { FaceEdit } from './RackDeviceFace';
import type { Dragging } from './useRackDrag';

interface Props {
  rack: Rack;
  nodes: Record<string, RackNode>;
  status: RackStatus | null;
  onOpen: (device: RackDevice) => void;
  /** Given while positions are being edited. */
  edit?: CabinetEdit;
}

export interface CabinetEdit extends Pick<FaceEdit, 'onPointerDown' | 'onKeyDown'> {
  /** Devices that are somewhere else than in the saved layout. */
  moved: Set<string>;
  /** Where moved devices were in this rack, drawn faintly. */
  origins: { id: string; label: string; u: number; h: number }[];
  dragging: Dragging | null;
}

/** One cabinet: slot numbers down the side (U1 at the bottom), devices to scale, and under it the devices nobody has placed yet. */
export default function RackCabinet({ rack, nodes, status, onOpen, edit }: Props) {
  const { t } = useT();
  const { placed, unplaced } = useMemo(() => arrange(rack.devices, rack.units), [rack]);
  const numbers = useMemo(() => slotNumbers(rack.units), [rack.units]);
  const face = (device: RackDevice, units: number | null) => (
    <RackDeviceFace key={device.id} device={device} look={lookOf(device, nodes)} rackName={rack.name} units={units} status={status} onOpen={onOpen}
      edit={edit && { moved: edit.moved.has(device.id), held: edit.dragging?.id === device.id, onPointerDown: edit.onPointerDown, onKeyDown: edit.onKeyDown }} />
  );
  const box = (u: number, h: number) => { const b = deviceBox(u, h, rack.units); return { '--top': b.top, '--h': b.height } as React.CSSProperties; };
  const drop = edit?.dragging?.target?.rack === rack.id ? edit.dragging : null;
  const refused = drop?.target && !drop.target.check.ok ? drop.target.check : null;
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
        <div className="rack-slots" data-rack-slots={rack.id}>
          {edit?.origins.map((o) => (
            <div key={o.id} className="rack-origin" style={box(o.u, o.h)} data-origin={o.id} title={t('rack.edit.origin', { label: o.label })} aria-hidden="true"><span>{o.label}</span></div>
          ))}
          {placed.map((device) => face(device, rack.units))}
          {drop?.target && (
            <div className={`rack-drop ${refused ? 'is-invalid' : 'is-valid'}`} style={box(drop.target.u, drop.h)} data-drop={refused ? 'invalid' : 'valid'}>
              <strong>{drop.label}</strong>
              <span>{refused ? (refused.reason === 'overlap' ? t('rack.edit.drop.overlap', { other: refused.other ?? '' }) : t('rack.edit.drop.outside')) : slotLabel(drop.target.u, drop.h)}</span>
            </div>
          )}
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
