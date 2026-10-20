import React, { useEffect, useState } from 'react';
import { useT } from '../../../../i18n';

interface Props {
  url: string;
  kind: 'video' | 'audio';
  name: string;
  /** Length and picture size for the header, once known. */
  onFacts: (facts: string) => void;
}

const clock = (sec: number) => {
  if (!Number.isFinite(sec)) return '';
  const s = Math.round(sec);
  const h = Math.floor(s / 3600);
  const mm = String(Math.floor((s % 3600) / 60)).padStart(h ? 2 : 1, '0');
  return `${h ? `${h}:` : ''}${mm}:${String(s % 60).padStart(2, '0')}`;
};

/**
 * A video or a song, played by the browser straight from the server: it
 * fetches only the ranges it plays, so a large file starts at once and can
 * be seeked anywhere. Formats the browser cannot play (some MKV or MOV
 * codecs) say so, and can still be downloaded.
 */
export default function MediaPreview({ url, kind, name, onFacts }: Props) {
  const { t } = useT();
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [url]);

  if (failed) return <p className="preview-note" role="alert">{t('preview.mediaUnsupported')}</p>;

  const facts = (e: React.SyntheticEvent<HTMLMediaElement>) => {
    const m = e.currentTarget;
    const size = m instanceof HTMLVideoElement && m.videoWidth ? ` · ${m.videoWidth} × ${m.videoHeight}` : '';
    onFacts(`${clock(m.duration)}${size}`);
  };

  if (kind === 'video') {
    return (
      <video className="preview-video" src={url} controls autoPlay playsInline preload="metadata"
        onLoadedMetadata={facts} onError={() => setFailed(true)} aria-label={name} />
    );
  }
  return (
    <div className="preview-audio">
      <div className="preview-audio-icon" aria-hidden="true">♪</div>
      <div className="preview-audio-name">{name}</div>
      <audio src={url} controls autoPlay preload="metadata" onLoadedMetadata={facts} onError={() => setFailed(true)} aria-label={name} />
    </div>
  );
}
