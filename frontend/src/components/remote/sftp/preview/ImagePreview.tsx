import React, { useEffect, useState } from 'react';
import { useT } from '../../../../i18n';

interface Props {
  blob: Blob;
  name: string;
  /** Fit the window, or show the image at its own size (scrolling). */
  fit: boolean;
  onToggleFit: () => void;
  /** The picture's own size, once it has decoded. */
  onSize: (size: { width: number; height: number } | null) => void;
}

/** An image, fitted to the window or at 100%, on a checkerboard so transparency shows. */
export default function ImagePreview({ blob, name, fit, onToggleFit, onSize }: Props) {
  const { t } = useT();
  const [url, setUrl] = useState('');
  const [broken, setBroken] = useState(false);

  useEffect(() => {
    const made = URL.createObjectURL(blob);
    setUrl(made);
    setBroken(false);
    return () => URL.revokeObjectURL(made);
  }, [blob]);

  if (broken) return <p className="preview-note">{t('preview.cannotDecode')}</p>;
  if (!url) return null;
  return (
    <div className={`preview-image ${fit ? 'is-fit' : 'is-actual'}`}>
      <img src={url} alt={name} draggable={false}
        title={t(fit ? 'preview.actualSizeHint' : 'preview.fitHint')}
        onClick={onToggleFit}
        onLoad={(e) => onSize({ width: e.currentTarget.naturalWidth, height: e.currentTarget.naturalHeight })}
        onError={() => { setBroken(true); onSize(null); }} />
    </div>
  );
}
