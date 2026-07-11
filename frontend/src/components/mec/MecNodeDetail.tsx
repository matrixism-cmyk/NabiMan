import React, { useState } from 'react';
import { useT } from '../../i18n';
import { useMecApi, mecPost, mecPatch } from '../../hooks/mec/useMecApi';
import { MecNode, Taint } from '../../types/mec';
import NodeGpuModeForm from './NodeGpuModeForm';
import NodeTaintsEditor from './NodeTaintsEditor';
import NodeLabelsEditor from './NodeLabelsEditor';

interface Props {
  nodeName: string;
  onClose: () => void;
}

export default function MecNodeDetail({ nodeName, onClose }: Props) {
  const { t } = useT();
  const { data, loading, error, refetch } = useMecApi<MecNode>(
    `/api/mec/v1/nodes/${encodeURIComponent(nodeName)}`,
    30_000,
  );
  const [actionErr, setActionErr] = useState<string | null>(null);

  if (loading && !data) return <div className="panel-loading">{t('mec.state.loading')}</div>;
  if (error) return <div className="panel-error">{error}</div>;
  if (!data) return <div className="panel-empty">{t('mec.node.noInfo')}</div>;

  const node = data;

  const handleSwitchGpu = async (mode: string, replicas?: number) => {
    setActionErr(null);
    try {
      const res = await mecPost(
        `/api/mec/v1/nodes/${encodeURIComponent(nodeName)}/gpu-mode`,
        { mode, replicas: replicas ?? null, force: false },
      );
      if (res.error) setActionErr(res.error.message);
      else await refetch();
    } catch (e) {
      setActionErr(String(e));
    }
  };

  const handleTaintPatch = async (
    add: Taint[],
    remove: { key: string; effect?: string }[],
  ) => {
    setActionErr(null);
    try {
      const res = await mecPatch(
        `/api/mec/v1/nodes/${encodeURIComponent(nodeName)}/taints`,
        {
          add: add.length ? add : null,
          remove: remove.length ? remove : null,
        },
      );
      if (res.error) setActionErr(res.error.message);
      else await refetch();
    } catch (e) {
      setActionErr(String(e));
    }
  };

  const handleLabelPatch = async (
    add: Record<string, string>,
    remove: string[],
  ) => {
    setActionErr(null);
    try {
      const res = await mecPatch(
        `/api/mec/v1/nodes/${encodeURIComponent(nodeName)}/labels`,
        {
          add: Object.keys(add).length ? add : null,
          remove: remove.length ? remove : null,
        },
      );
      if (res.error) setActionErr(res.error.message);
      else await refetch();
    } catch (e) {
      setActionErr(String(e));
    }
  };

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('mec.node.detailTitle', { name: node.name })}</h2>
        <button className="btn btn-secondary" onClick={onClose}>
          {t('mec.node.backToList')}
        </button>
      </div>

      {actionErr && <div className="panel-error">{actionErr}</div>}

      <div className="panel-card" style={{ marginBottom: '16px' }}>
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))',
            gap: '12px',
          }}
        >
          <Info label={t('mec.node.info.status')} value={node.status} />
          <Info label={t('mec.node.info.roles')} value={node.roles.join(', ') || 'worker'} />
          <Info label="CPU" value={node.capacity.cpu} />
          <Info label="Memory" value={node.capacity.memory} />
          <Info label={t('mec.node.info.arch')} value={node.architecture} />
          <Info label="OS" value={node.os_image} />
          <Info label="Kernel" value={node.kernel_version} />
          <Info
            label={t('mec.node.info.cudaDriver')}
            value={node.cuda_driver || '-'}
          />
          <Info
            label={t('mec.node.info.currentTenant')}
            value={node.current_tenant || t('mec.node.info.noTenant')}
          />
        </div>
      </div>

      {node.gpu_info && (
        <div className="panel-card" style={{ marginBottom: '16px' }}>
          <h3 style={{ marginTop: 0 }}>GPU</h3>
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(160px, 1fr))',
              gap: '12px',
              marginBottom: '12px',
            }}
          >
            <Info label={t('mec.node.gpu.model')} value={node.gpu_info.model} />
            <Info label={t('mec.node.gpu.physicalCards')} value={String(node.gpu_info.count)} />
            <Info
              label={t('mec.node.gpu.totalSlots')}
              value={String(node.gpu_info.total_slots)}
            />
            <Info label={t('mec.node.gpu.currentMode')} value={node.gpu_info.mode} />
            <Info
              label={t('mec.node.gpu.sharingStrategy')}
              value={node.gpu_info.sharing_strategy || '-'}
            />
            <Info
              label="Replicas"
              value={String(node.gpu_info.replicas ?? '-')}
            />
          </div>
          <NodeGpuModeForm
            currentMode={node.gpu_info.mode}
            onSwitch={handleSwitchGpu}
          />
        </div>
      )}

      <div className="panel-card" style={{ marginBottom: '16px' }}>
        <h3 style={{ marginTop: 0 }}>Taints</h3>
        <NodeTaintsEditor taints={node.taints} onPatch={handleTaintPatch} />
      </div>

      <div className="panel-card">
        <h3 style={{ marginTop: 0 }}>Labels</h3>
        <NodeLabelsEditor labels={node.labels} onPatch={handleLabelPatch} />
      </div>
    </div>
  );
}

function Info({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div style={{ fontSize: '11px', color: '#6b7280', textTransform: 'uppercase' }}>
        {label}
      </div>
      <div style={{ fontSize: '15px', fontWeight: 500 }}>{value}</div>
    </div>
  );
}
