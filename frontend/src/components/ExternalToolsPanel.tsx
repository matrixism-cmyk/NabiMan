import React, { useState } from 'react';
import { apiRequest, useApi } from '../shared/api/useApi';
import { useT } from '../i18n';

/** One optional program, as the server found it at startup (backend/src/external_tools). */
export interface ExternalTool {
  name: string;
  found: boolean;
  path: string | null;
  version: string | null;
  features: string[];
}
interface Runtime { tools: { checked_at: string; tools: ExternalTool[] } | null }

/** Which optional programs the server has, and what is off for lack of one. */
export default function ExternalToolsPanel() {
  const { t } = useT();
  const { data, error, refetch } = useApi<Runtime>('/api/diagnostics/runtime');
  const [checking, setChecking] = useState(false);
  const recheck = async () => {
    setChecking(true);
    // The check runs in the background on the server; its result shows a moment later.
    await apiRequest('/api/diagnostics/runtime?refresh=1');
    setTimeout(() => { refetch(); setChecking(false); }, 1500);
  };
  const report = data?.tools;
  return <section aria-label={t('diagnostics.tools')}>
    <h3>{t('diagnostics.tools')}</h3>
    <p className="text-secondary">{t('diagnostics.toolsHint')}</p>
    {error && <p role="alert">{error}</p>}
    {!error && data && !report && <p>{t('diagnostics.toolsNone')}</p>}
    {report && <>
      <table className="data-table">
        <thead><tr><th>{t('diagnostics.toolName')}</th><th>{t('diagnostics.toolStatus')}</th><th>{t('diagnostics.toolFeatures')}</th></tr></thead>
        <tbody>{report.tools.map((tool) => <tr key={tool.name}>
          <td><code>{tool.name}</code>{tool.version && tool.version !== tool.name && <> <small className="text-secondary">{tool.version}</small></>}</td>
          <td><span className={`status-badge ${tool.found ? 'up' : tool.features.length ? 'down' : ''}`} title={tool.path ?? undefined}>
            {tool.found ? t('diagnostics.toolFound') : tool.features.length ? t('diagnostics.toolMissing') : t('diagnostics.toolUnused')}
          </span></td>
          <td>{tool.features.map((f) => t(`diagnostics.feature.${f}`)).join(', ') || '—'}</td>
        </tr>)}</tbody>
      </table>
      <p className="text-secondary">{t('diagnostics.toolsCheckedAt', { at: report.checked_at })}</p>
    </>}
    <button className="btn btn-secondary" onClick={recheck} disabled={checking}>{t('diagnostics.toolsRecheck')}</button>
  </section>;
}
