import React from 'react';
import { useT } from '../../i18n';
import type { Level, NodeStatus, RackDevice, RackStatus } from './rackTypes';
import { CALM_GLYPH, LEVEL_GLYPH, Look, celsius, deviceNodes, gpuCount, gpuPeak, hottest, isCalm, pct, reachText } from './rackFormat';
import { deviceBox, slotLabel } from './rackGeometry';

interface Props {
  device: RackDevice;
  look: Look;
  rackName: string;
  /** Rack height in slots when the device is drawn in the rack; null when it is only listed under it. */
  units: number | null;
  status: RackStatus | null;
  onOpen: (device: RackDevice) => void;
}

/** The level to show for a device before or without a status answer. */
export function levelOf(device: RackDevice, status: RackStatus | null): Level {
  return status?.devices[device.id]?.level ?? (deviceNodes(device).length ? 'unknown' : 'none');
}

/** `calm`: no reply from a device that is not ours. */
export function Light({ level, calm = false }: { level: Level; calm?: boolean }) {
  const { t } = useT();
  const name = calm ? t('rack.level.externalNoreply') : t(`rack.level.${level}`);
  return <span className={`rack-light level-${level}${calm ? ' is-calm' : ''}`} role="img" aria-label={name} title={name}>{calm ? CALM_GLYPH : LEVEL_GLYPH[level]}</span>;
}

/** One node's numbers, short enough for a device's face. */
function Readings({ node, status }: { node: NodeStatus; status: RackStatus }) {
  const { t } = useT();
  const hot = hottest(node);
  const gpus = gpuCount(node);
  const over = (value: number | null, at: number) => (value !== null && value >= at ? ' is-over' : '');
  return (
    <>
      <span className="rack-reading"><b>CPU</b> {pct(node.cpu_pct)}</span>
      <span className={`rack-reading${over(node.mem_pct, status.thresholds.memory_pct)}`}><b>{t('rack.face.memory')}</b> {pct(node.mem_pct)}</span>
      <span className={`rack-reading${over(hot, status.thresholds.temp_c)}`}>{celsius(hot)}</span>
      {gpus > 0 && <span className="rack-reading"><b>{t('rack.face.gpu', { count: gpus })}</b> {pct(gpuPeak(node))}</span>}
    </>
  );
}

/**
 * A device as a button: its status light, name, node, and for a server its
 * live numbers. Drawn to scale in the rack (`units` given) or as a card in
 * the list under it.
 */
export default function RackDeviceFace({ device, look, rackName, units, status, onOpen }: Props) {
  const { t } = useT();
  const level = levelOf(device, status);
  const nodes = deviceNodes(device);
  const undecided = !device.node && nodes.length > 0;
  const placed = units !== null && device.u !== null;
  const box = placed ? deviceBox(device.u as number, device.h, units as number) : null;
  const where = device.u !== null ? `${rackName} ${slotLabel(device.u, device.h)}` : `${rackName} ${t('rack.unplaced')}`;
  const style = box ? ({ '--top': box.top, '--h': box.height } as React.CSSProperties) : undefined;
  const own = device.node && status ? status.nodes[device.node] : undefined;
  const tall = !placed || device.h >= 2;
  const ownStatus = status?.devices[device.id];
  const calm = isCalm(ownStatus);
  const reach = reachText(t, ownStatus);
  const reachTag = reach ? <span className={`rack-reach level-${level}${calm ? ' is-calm' : ''}`} data-reach>{reach}</span> : null;

  return (
    <button
      type="button"
      className={`rack-device look-${look} level-${level}${calm ? ' is-calm' : ''}${device.u_confirmed ? '' : ' is-unconfirmed'}${placed ? '' : ' is-listed'}${tall ? ' is-tall' : ''}`}
      style={style}
      data-device={device.id}
      data-level={level}
      aria-label={t('rack.device.aria', { label: device.label, where, level: reach || t(`rack.level.${level}`) })}
      title={`${device.label} · ${where}`}
      onClick={() => onOpen(device)}
    >
      <span className="rack-device-head">
        <Light level={level} calm={calm} />
        <span className="rack-device-label">{device.label}</span>
        {!tall && reachTag}
        {device.node && <span className="rack-device-node">{device.node}</span>}
        {undecided && <span className="rack-device-node is-undecided">{t('rack.nodeUndecided')}</span>}
        {!device.node && !undecided && device.ip && <span className="rack-device-node is-address">{device.ip}</span>}
      </span>
      {tall && reachTag && <span className="rack-device-readings">{reachTag}</span>}
      {own && status && tall && (
        <span className="rack-device-readings"><Readings node={own} status={status} /></span>
      )}
      {undecided && status && tall && (
        <span className="rack-device-readings">
          {nodes.map((name) => {
            const node = status.nodes[name];
            return (
              <span key={name} className="rack-candidate">
                <Light level={node?.level ?? 'unknown'} /> {name}
                {node && <> <span className="rack-reading is-extra"><b>CPU</b> {pct(node.cpu_pct)}</span> <span className="rack-reading">{celsius(hottest(node))}</span></>}
              </span>
            );
          })}
        </span>
      )}
    </button>
  );
}
