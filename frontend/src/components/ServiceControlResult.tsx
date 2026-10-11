import React from 'react';
import { useT } from '../i18n';

export interface ServiceReceipt {
  operation_id: string;
  unit: string;
  action: string;
  state: 'running' | 'command_succeeded' | 'command_failed' | 'not_dispatched' | 'unknown_result';
  exit_code: number | null;
}
export interface ServiceControlResponse { operation_id: string; receipt: ServiceReceipt | null }
interface Props {
  operationId: string;
  setOperationId: (id: string) => void;
  result: ServiceControlResponse | null;
  busy: boolean;
  inspect: () => void;
}
export default function ServiceControlResult({ operationId, setOperationId, result, busy, inspect }: Props) {
  const { t } = useT();
  return <section aria-label={t('services.operation')} className="message">
    <label>{t('services.operation')} <input value={operationId} onChange={e => setOperationId(e.target.value)} disabled={busy} /></label>
    <button type="button" className="btn btn-secondary btn-sm" disabled={busy || !operationId} onClick={inspect}>{t('services.inspect')}</button>
    {result?.receipt && <p>{result.receipt.unit} · {result.receipt.action} · {t(`services.outcome.${result.receipt.state}`)}</p>}
    <p>{t('services.outcomeHint')}</p>
  </section>;
}
