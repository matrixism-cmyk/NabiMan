import React, { useEffect, useState } from 'react';
import Modal from '../../shared/ui/Modal';
import { useT } from '../../shared/i18n';
import { copyText } from './ClaudeAddress';
import { fetchGuide, OrchestrationGuide } from './aiGuide';

/** Why a guide could not be fetched, as the server put it (may be empty). */
export const guideFailure = (e: unknown) => (e instanceof Error ? e.message : '');

interface Props {
  sessionId: string;
  onClose: () => void;
}

/** The guide as it would be pasted: read-only, scrollable, with its own copy button. */
export default function AiGuidePreview({ sessionId, onClose }: Props) {
  const { t } = useT();
  const [guide, setGuide] = useState<OrchestrationGuide | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [copied, setCopied] = useState<'ok' | 'failed' | null>(null);

  useEffect(() => {
    let live = true;
    fetchGuide(sessionId)
      .then((g) => { if (live) setGuide(g); })
      .catch((e) => { if (live) setFailure(guideFailure(e)); });
    return () => { live = false; };
  }, [sessionId]);

  const copy = async () => {
    if (!guide) return;
    setCopied((await copyText(guide.text)) ? 'ok' : 'failed');
  };

  return (
    <Modal onClose={onClose} title={t('aiGuide.previewTitle')} className="ai-guide-preview">
      {failure !== null ? (
        <p role="alert" className="ai-guide-note is-error">{t('aiGuide.loadFailed')}{failure && ` (${failure})`}</p>
      ) : !guide ? (
        <p className="ai-guide-note">{t('aiGuide.loading')}</p>
      ) : (
        <>
          <pre className="ai-guide-text" tabIndex={0} aria-label={t('aiGuide.text')} data-testid="ai-guide-text">{guide.text}</pre>
          {guide.updated && <p className="ai-guide-updated">{t('aiGuide.updated', { date: guide.updated })}</p>}
        </>
      )}
      <div className="btn-group ai-guide-actions">
        <button type="button" className="btn btn-primary" onClick={copy} disabled={!guide}>{t('aiGuide.copy')}</button>
        <span role="status" className={`ai-guide-note ${copied === 'failed' ? 'is-error' : 'is-ok'}`}>
          {copied === 'ok' ? t('aiGuide.copied') : copied === 'failed' ? t('aiGuide.copyFailed') : ''}
        </span>
        <button type="button" className="btn btn-secondary" onClick={onClose}>{t('common.close')}</button>
      </div>
    </Modal>
  );
}
