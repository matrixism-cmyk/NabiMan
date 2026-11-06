import React, { useEffect, useState } from 'react';
import { useT } from '../../../../shared/i18n';
import { fetchWithRefresh } from '../../../../shared/api/useApi';
import { API_BASE } from '../sftpApi';
import PdfPreview from './PdfPreview';

interface Props {
  blob: Blob;
  name: string;
  /** Office previews are made on the server, for signed-in accounts only. */
  allowed: boolean;
  onInfo: (info: { pages: number } | null) => void;
}

type State = { state: 'converting' } | { state: 'ready'; pdf: Blob } | { state: 'failed'; message: string };

/**
 * A Word, Excel, PowerPoint or OpenDocument file: the server turns it into a
 * PDF (LibreOffice), shown by the same viewer as any PDF.
 */
export default function OfficePreview({ blob, name, allowed, onInfo }: Props) {
  const { t } = useT();
  const [now, setNow] = useState<State>({ state: 'converting' });

  useEffect(() => {
    if (!allowed) { setNow({ state: 'failed', message: t('preview.officeSignedIn') }); return undefined; }
    const abort = new AbortController();
    setNow({ state: 'converting' });
    fetchWithRefresh(`${API_BASE}/api/preview/office?name=${encodeURIComponent(name)}`, {
      method: 'POST', body: blob, signal: abort.signal, headers: { 'Content-Type': 'application/octet-stream' },
    })
      .then(async (res) => {
        if (res.ok && (res.headers.get('Content-Type') || '').startsWith('application/pdf')) {
          setNow({ state: 'ready', pdf: new Blob([await res.arrayBuffer()], { type: 'application/pdf' }) });
          return;
        }
        let message = t('preview.officeFailed');
        try { const json = await res.json(); if (json?.message) message = json.message; } catch { /* not JSON */ }
        setNow({ state: 'failed', message });
      })
      .catch((e: unknown) => {
        if (e instanceof DOMException && e.name === 'AbortError') return;
        setNow({ state: 'failed', message: t('preview.officeFailed') });
      });
    return () => abort.abort();
  }, [blob, name, allowed, t]);

  if (now.state === 'converting') return <p className="preview-note">{t('preview.officeConverting')}</p>;
  if (now.state === 'failed') return <p className="preview-note" role="alert">{now.message}</p>;
  // A converted page can be tiny (a sheet with a few cells): no more than one and a half times its size.
  return <PdfPreview blob={now.pdf} onInfo={onInfo} maxFit={1.5} />;
}
