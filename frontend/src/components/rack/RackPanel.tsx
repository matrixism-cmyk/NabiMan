import React, { useCallback, useMemo, useState } from 'react';
import { useT } from '../../i18n';
import { useApi } from '../../shared/api/useApi';
import type { LayoutAnswer, Rack, RackDevice, StatusAnswer } from './rackTypes';
import { lookOf, nodesByName } from './rackFormat';
import RackCabinet from './RackCabinet';
import RackDetail from './RackDetail';
import { RackLegend, RackMeta, RackSummary } from './RackHeader';
import './rack.css';

/** How often the page asks again; `useApi` rests while the tab is hidden and catches up when it shows. */
export const REFRESH_MS = 30_000;

function Notice({ tone, title, children }: { tone: 'info' | 'warning' | 'error'; title?: string; children?: React.ReactNode }) {
  return (
    <div className={`rack-banner tone-${tone}`} role={tone === 'error' ? 'alert' : 'status'}>
      {title && <strong>{title}</strong>}
      {children}
    </div>
  );
}

/**
 * The server room at a glance: each rack drawn as the layout file has it,
 * every device with its status light, servers with their live numbers.
 * Read-only; a device opens its details.
 */
export default function RackPanel() {
  const { t } = useT();
  const layoutApi = useApi<LayoutAnswer>('/api/rack/layout', REFRESH_MS);
  const statusApi = useApi<StatusAnswer>('/api/rack/status', REFRESH_MS);
  const [openId, setOpenId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const layout = layoutApi.data?.layout ?? null;
  // A status for another version of the layout would light the wrong devices.
  const status = statusApi.data?.status && layout && statusApi.data.status.layout_version === layout.version ? statusApi.data.status : null;
  const nodes = useMemo(() => (layout ? nodesByName(layout) : {}), [layout]);
  const open = useMemo(() => {
    if (!layout || !openId) return null;
    for (const rack of layout.racks) {
      const device = rack.devices.find((d) => d.id === openId);
      if (device) return { rack, device } as { rack: Rack; device: RackDevice };
    }
    return null;
  }, [layout, openId]);
  const onOpen = useCallback((device: RackDevice) => setOpenId(device.id), []);
  const refresh = async () => {
    setBusy(true);
    try { await Promise.all([layoutApi.refetch(), statusApi.refetch()]); } finally { setBusy(false); }
  };

  const off = layoutApi.data?.configured === false;
  const fault = layoutApi.data?.error ?? null;
  const tentative = Boolean(layout?.racks.some((r) => r.devices.some((d) => !d.u_confirmed)));
  const title = layout?.title || t('rack.title');

  return (
    <div className="panel rack-panel">
      <div className="rack-top">
        <div className="rack-heading">
          <h2>{title}</h2>
          {layout && <RackMeta layout={layout} status={status} tentative={tentative} />}
        </div>
        {status && <RackSummary status={status} />}
        {!off && (
          <button type="button" className="btn btn-secondary btn-sm rack-refresh" onClick={refresh} disabled={busy}>
            {t('rack.refresh')}
          </button>
        )}
      </div>

      {!layoutApi.data && layoutApi.loading && <div className="rack-state" role="status">{t('rack.loading')}</div>}
      {!layoutApi.data && layoutApi.error && <Notice tone="error">{t('rack.requestFailed', { message: layoutApi.error })}</Notice>}
      {off && (
        <div className="rack-state" data-state="off">
          <strong>{t('rack.off.title')}</strong>
          <p>{t('rack.off.body')}</p>
        </div>
      )}
      {fault && (
        <div className="rack-state" data-state="fault" role="alert">
          <strong>{t(`rack.fault.${['no_file', 'unreadable', 'invalid'].includes(fault.code) ? fault.code : 'unreadable'}`)}</strong>
          <code>{fault.detail}</code>
          <p>{t('rack.fault.hint')}</p>
        </div>
      )}

      {layout && (
        <>
          {statusApi.error && !status && <Notice tone="warning">{t('rack.requestFailed', { message: statusApi.error })}</Notice>}
          {status && status.source !== 'ok' && (
            <Notice tone="warning" title={t(`rack.source.${status.source}`)}>
              {status.message && <code>{status.message}</code>}
            </Notice>
          )}
          {layout.overlaps.length > 0 && (
            <Notice tone="warning">{t('rack.overlap', { list: layout.overlaps.map((o) => `${o.rack}: ${o.first} / ${o.second}`).join(', ') })}</Notice>
          )}
          <div className="rack-grid" data-racks={layout.racks.length}>
            {layout.racks.map((rack) => <RackCabinet key={rack.id} rack={rack} nodes={nodes} status={status} onOpen={onOpen} />)}
          </div>
          <RackLegend status={status} tentative={tentative} />
        </>
      )}

      {open && (
        <RackDetail device={open.device} rack={open.rack} look={lookOf(open.device, nodes)} nodes={nodes} status={status} onClose={() => setOpenId(null)} />
      )}
    </div>
  );
}
