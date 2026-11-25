import React, { useRef } from 'react';
import { useT } from '../../i18n';
import Modal from '../../shared/ui/Modal';
import type { NodeStatus, Rack, RackDevice, RackNode, RackStatus } from './rackTypes';
import { Look, barTone, celsius, deviceNodes, gib, gpuName, pct, watts } from './rackFormat';
import { slotLabel } from './rackGeometry';
import { Light, levelOf } from './RackDeviceFace';

interface Props {
  device: RackDevice;
  rack: Rack;
  look: Look;
  nodes: Record<string, RackNode>;
  status: RackStatus | null;
  onClose: () => void;
}

function Fact({ label, children }: { label: string; children: React.ReactNode }) {
  if (children === null || children === undefined || children === '') return null;
  return (<><dt>{label}</dt><dd>{children}</dd></>);
}

/** A labelled value with a small bar; `max` 100 for percentages. */
function Bar({ label, value, text, warnAt, max = 100 }: { label: string; value: number | null; text: string; warnAt: number | null; max?: number }) {
  const width = value === null ? 0 : Math.max(0, Math.min(100, (value / max) * 100));
  const tone = barTone(value, warnAt);
  return (
    <div className={`rack-bar tone-${tone}`}>
      <span className="rack-bar-label">{label}</span>
      <span className="rack-bar-track" role="img" aria-label={`${label} ${text}`}><span style={{ width: `${width}%` }} /></span>
      <span className="rack-bar-value">{tone === 'warn' ? '▲ ' : ''}{text}</span>
    </div>
  );
}

function NodeCard({ name, info, node, status }: { name: string; info: RackNode | undefined; node: NodeStatus | undefined; status: RackStatus | null }) {
  const { t } = useT();
  const level = node?.level ?? 'unknown';
  const limits = status?.thresholds;
  const spec = [info?.cpu ? t('rack.detail.cores', { count: info.cpu }) : '', info?.mem_gi ? t('rack.detail.memGi', { count: info.mem_gi }) : '', info?.arch ?? '', info?.gpu ?? ''].filter(Boolean).join(' · ');
  return (
    <section className={`rack-node-card level-${level}`} data-node={name}>
      <h4><Light level={level} /> {name} <span className="rack-node-level">{t(`rack.level.${level}`)}</span></h4>
      {node && node.reasons.length > 0 && <p className="rack-node-reasons">▲ {node.reasons.map((r) => t(`rack.reason.${r}`)).join(' · ')}</p>}
      <dl className="rack-facts">
        <Fact label={t('rack.detail.model')}>{info?.model}</Fact>
        <Fact label={t('rack.detail.role')}>{info?.role}</Fact>
        <Fact label={t('rack.detail.ip')}>{info?.ip}</Fact>
        <Fact label={t('rack.detail.spec')}>{spec}</Fact>
      </dl>
      {node && (
        <>
          <Bar label={t('rack.detail.cpu')} value={node.cpu_pct} text={pct(node.cpu_pct)} warnAt={null} />
          <Bar label={t('rack.detail.memory')} value={node.mem_pct} text={pct(node.mem_pct)} warnAt={limits?.memory_pct ?? null} />
          <Bar label={t('rack.detail.disk')} value={node.disk_pct} text={pct(node.disk_pct)} warnAt={limits?.disk_pct ?? null} />
          <Bar label={t('rack.detail.boardTemp')} value={node.temp_c} text={celsius(node.temp_c)} warnAt={limits?.temp_c ?? null} />
          {node.gpus.length === 0 && <p className="rack-muted">{t('rack.detail.noGpu')}</p>}
          {node.gpus.map((gpu) => (
            <div key={`${gpu.index}-${gpu.mig_id ?? ''}`} className="rack-gpu">
              <h5>{gpuName(gpu)} <span>{[gpu.model, gpu.mig_profile].filter(Boolean).join(' · ')}</span></h5>
              <Bar label={t('rack.detail.gpuUtil')} value={gpu.util_pct} text={pct(gpu.util_pct)} warnAt={null} />
              <Bar label={t('rack.detail.gpuTemp')} value={gpu.temp_c} text={celsius(gpu.temp_c)} warnAt={limits?.temp_c ?? null} />
              <Bar label={t('rack.detail.gpuMem')} value={gpu.mem_used_mib} max={gpu.mem_total_mib || 1}
                text={`${gib(gpu.mem_used_mib)} / ${gib(gpu.mem_total_mib)} GiB`} warnAt={null} />
              <div className="rack-bar"><span className="rack-bar-label">{t('rack.detail.gpuPower')}</span><span className="rack-bar-value">{watts(gpu.power_w)}</span></div>
            </div>
          ))}
        </>
      )}
    </section>
  );
}

/** Everything known about one device; Esc or a click outside closes it. */
export default function RackDetail({ device, rack, look, nodes, status, onClose }: Props) {
  const { t } = useT();
  // Focus starts at the top, not on the Close button under everything.
  const top = useRef<HTMLDListElement>(null);
  const names = deviceNodes(device);
  const level = levelOf(device, status);
  const undecided = !device.node && names.length > 0;
  const position = device.u !== null ? t('rack.detail.rackU', { rack: rack.name, slot: `${slotLabel(device.u, device.h)} (${device.h}U)` }) : `${rack.name} · ${t('rack.unplaced')}`;
  return (
    <Modal onClose={onClose} className="rack-detail" initialFocus={top} title={<><Light level={level} /> {device.label}</>}>
      <dl className="rack-facts" ref={top} tabIndex={-1}>
        <Fact label={t('rack.detail.type')}>{t(`rack.look.${look}`)}</Fact>
        <Fact label={t('rack.detail.position')}>{position}</Fact>
        <Fact label={t('rack.detail.node')}>{device.node ?? (undecided ? `${t('rack.nodeUndecided')} (${names.join(', ')})` : null)}</Fact>
        {!device.node && <Fact label={t('rack.detail.ip')}>{device.ip}</Fact>}
        <Fact label={t('rack.detail.bmc')}>{device.bmc}</Fact>
        <Fact label={t('rack.detail.notes')}>{device.desc}</Fact>
      </dl>
      {device.u === null && <p className="rack-notice">{t('rack.detail.unplaced')}</p>}
      {device.u !== null && !device.u_confirmed && <p className="rack-notice">{t('rack.detail.uNotice')}</p>}
      {undecided && <p className="rack-notice">{t('rack.detail.candidates')}</p>}
      {names.length === 0 && <p className="rack-muted">{t('rack.detail.notCollected')}</p>}
      {names.length > 0 && (
        <div className="rack-node-cards">
          {names.map((name) => <NodeCard key={name} name={name} info={nodes[name]} node={status?.nodes[name]} status={status} />)}
        </div>
      )}
      <div className="rack-detail-actions">
        <button type="button" className="btn btn-secondary" onClick={onClose}>{t('rack.detail.close')}</button>
      </div>
    </Modal>
  );
}
