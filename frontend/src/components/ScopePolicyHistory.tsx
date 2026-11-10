import React from 'react';
import { useApi } from '../shared/api/useApi';
import { useT } from '../i18n';
import type { TFn } from '../i18n';
interface Change { revision: string; at: string; actor: string; actor_id: string; reason: string; fields: { field: string; before: unknown; after: unknown }[] }
const FIELDS = ['contact_email', 'member_notice', 'member_limit', 'allow_commands', 'require_mfa'];
function value(t: TFn, input: unknown): string {
  if (input === null || input === '') return t('scope.history.none');
  if (typeof input === 'boolean') return input ? t('scope.history.on') : t('scope.history.off');
  return String(input);
}
export default function ScopePolicyHistory({ endpoint }: { endpoint: string }) {
  const { data, loading, error, refetch } = useApi<Change[]>(`${endpoint}/history`);
  const { t } = useT();
  return <section aria-label={t('scope.history.title')}>
    <h3>{t('scope.history.title')}</h3>
    <p>{t('scope.history.lead')}</p>
    <button className="btn btn-secondary" onClick={refetch}>{t('scope.history.refresh')}</button>
    {loading && <p>{t('scope.history.loading')}</p>}
    {error && <p role="alert">{error}</p>}
    {data?.length === 0 && <p>{t('scope.history.empty')}</p>}
    {data?.map(change => <details key={change.revision}>
      <summary>{new Date(change.at).toLocaleString()} · {change.actor} · {change.reason}</summary>
      <p>{t('scope.history.by', { actor: change.actor, id: change.actor_id })}</p>
      {change.fields.length === 0 ? <p>{t('scope.history.noFields')}</p> : <table className="data-table"><thead><tr><th>{t('scope.history.field')}</th><th>{t('scope.history.before')}</th><th>{t('scope.history.after')}</th></tr></thead>
        <tbody>{change.fields.map(field => <tr key={field.field}><td>{FIELDS.includes(field.field) ? t(`scope.history.f.${field.field}`) : field.field}</td><td style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{value(t, field.before)}</td><td style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{value(t, field.after)}</td></tr>)}</tbody></table>}
    </details>)}
  </section>;
}
