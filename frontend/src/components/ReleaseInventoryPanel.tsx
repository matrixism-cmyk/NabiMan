import React, { useEffect, useState } from 'react';
import { useT } from '../i18n';
interface Inventory { schema_version: number; kind: string; components: number; artifacts: Record<string, { sha256: string }> }
export default function ReleaseInventoryPanel({ runtimeSha }: { runtimeSha?: string }) {
  const { t } = useT(); const [inventory, setInventory] = useState<Inventory | null>(null); const [failed, setFailed] = useState(false);
  useEffect(() => {
    const controller = new AbortController();
    fetch('/release-inventory.json', { cache: 'no-store', signal: controller.signal }).then(async response => {
      if (!response.ok) throw new Error('Unavailable');
      const value = await response.json() as Inventory;
      if (value.schema_version !== 1 || value.kind !== 'unsigned-packaging-inventory' || !Number.isInteger(value.components) || !/^[a-f0-9]{64}$/.test(value.artifacts?.['nabiman-server']?.sha256 || '')) throw new Error('Invalid inventory');
      if (!controller.signal.aborted) setInventory(value);
    }).catch(() => { if (!controller.signal.aborted) setFailed(true); });
    return () => controller.abort();
  }, []);
  return <section aria-label={t('release.title')} style={{ marginTop: 20 }}>
    <h3>{t('release.title')}</h3><p>{t('release.limits')}</p>
    {failed && <p role="alert">{t('release.unavailable')}</p>}
    {inventory && <>
      <p>{t('release.components')}: {inventory.components}</p>
      <p role={runtimeSha && runtimeSha !== inventory.artifacts['nabiman-server'].sha256 ? 'alert' : 'status'}>{!runtimeSha ? t('release.unchecked') : runtimeSha === inventory.artifacts['nabiman-server'].sha256 ? t('release.match') : t('release.mismatch')}</p>
      <p><a href="/release-bom.cdx.json" target="_blank" rel="noreferrer">{t('release.sbom')}</a> · <a href="/release-inventory.json" target="_blank" rel="noreferrer">{t('release.receipt')}</a></p>
    </>}
  </section>;
}
