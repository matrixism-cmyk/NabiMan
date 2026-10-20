import { useEffect, useRef, useState } from 'react';
import { apiRequest, useApi } from './useApi';
import { ServiceControlResponse } from '../components/ServiceControlResult';
import { useT } from '../i18n';
import { confirmDialog } from '../components/common/ConfirmDialog';

export function useServiceControl(refetch: () => void) {
  const { t } = useT();
  const { data: me } = useApi<{ id: string }>('/api/users/me');
  const [operationId, setOperationId] = useState('');
  const [result, setResult] = useState<ServiceControlResponse | null>(null);
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const pending = useRef(false);
  const accountGeneration = useRef(0);
  useEffect(() => {
    accountGeneration.current += 1;
    pending.current = false;
    setResult(null); setMessage(''); setBusy(false);
    try { setOperationId(me?.id ? sessionStorage.getItem(`nabiman_service_operation:${me.id}`) || '' : ''); }
    catch { setOperationId(''); }
    return () => { accountGeneration.current += 1; };
  }, [me?.id]);
  const remember = (id: string) => {
    setOperationId(id);
    if (me?.id) {
      try { sessionStorage.setItem(`nabiman_service_operation:${me.id}`, id); }
      catch { /* The current operation remains visible when browser storage is unavailable. */ }
    }
  };
  const apply = (response: Awaited<ReturnType<typeof apiRequest<ServiceControlResponse>>>) => {
    if (response.data) { setResult(response.data); remember(response.data.operation_id); }
    setMessage(response.success && response.data?.receipt ? t(`services.outcome.${response.data.receipt.state}`) : response.message);
  };
  const act = async (name: string, action: string) => {
    if (pending.current || !me?.id) return;
    if ((action === 'stop' || action === 'disable') && !(await confirmDialog({ message: `${action} "${name}"?`, danger: true }))) return;
    const id = crypto.randomUUID();
    const generation = accountGeneration.current;
    pending.current = true;
    remember(id); setResult(null); setBusy(true); setMessage('');
    try {
      const response = await apiRequest<ServiceControlResponse>('/api/services/action', {
        method: 'POST', headers: { 'Idempotency-Key': id }, body: JSON.stringify({ name, action }),
      });
      if (generation === accountGeneration.current) apply(response);
    } catch { if (generation === accountGeneration.current) setMessage(t('services.unknown')); }
    finally {
      if (generation === accountGeneration.current) { pending.current = false; setBusy(false); refetch(); }
    }
  };
  const inspect = async () => {
    if (pending.current || !operationId || !me?.id) return;
    const generation = accountGeneration.current;
    pending.current = true;
    setBusy(true); setMessage(''); setResult(null);
    try {
      const response = await apiRequest<ServiceControlResponse>(`/api/services/operations/${encodeURIComponent(operationId)}`);
      if (generation === accountGeneration.current) apply(response);
    }
    catch { if (generation === accountGeneration.current) setMessage(t('services.inspectFailed')); }
    finally {
      if (generation === accountGeneration.current) { pending.current = false; setBusy(false); }
    }
  };
  return { act, inspect, operationId, setOperationId, result, message, busy, ready: Boolean(me?.id) };
}
