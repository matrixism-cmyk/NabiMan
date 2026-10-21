import React from 'react';
import { useApi } from '../shared/api/useApi';
import { useT } from '../i18n';
import ReleaseInventoryPanel from './ReleaseInventoryPanel';

interface RuntimeIdentity {
  package_version: string;
  executable_sha256: string;
  instance_id: string;
  started_at: string;
}

export default function RuntimeIdentityPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<RuntimeIdentity>('/api/diagnostics/runtime');
  return <section aria-label={t('diagnostics.runtime')}>
    <h3>{t('diagnostics.runtime')}</h3>
    <p className="text-secondary">{t('diagnostics.runtimeHint')}</p>
    {loading && <p>{t('common.loading')}</p>}
    {error && <p role="alert">{error}</p>}
    {!error && data && <dl>
      <dt>{t('diagnostics.packageVersion')}</dt><dd>{data.package_version}</dd>
      <dt>{t('diagnostics.startedAt')}</dt><dd><time dateTime={data.started_at}>{data.started_at}</time></dd>
      <dt>{t('diagnostics.instanceId')}</dt><dd><code>{data.instance_id}</code></dd>
      <dt>{t('diagnostics.executableHash')}</dt><dd><code style={{ overflowWrap: 'anywhere' }}>{data.executable_sha256}</code></dd>
    </dl>}
    <button className="btn btn-secondary" onClick={refetch}>{t('common.refresh')}</button>
    <ReleaseInventoryPanel runtimeSha={error ? undefined : data?.executable_sha256} />
  </section>;
}
