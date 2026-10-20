import React, { useEffect, useRef, useState } from 'react';
import type { PDFDocumentProxy } from './pdf';

interface Props {
  doc: PDFDocumentProxy;
  /** 1-based. */
  number: number;
  scale: number;
  /** Size at scale 1 to hold the place until this page's own is known. */
  guess: { width: number; height: number };
  /** The scrolling box, for telling when the page comes near. */
  root: HTMLElement | null;
  textLayer: typeof import('./pdf').TextLayer;
}

/** How far outside the view a page is drawn ahead of being scrolled to. */
const AHEAD = '800px 0px';

/**
 * One page: a placeholder of the right size, drawn on a canvas only while it
 * is near the view (and let go again when it is far), with a transparent
 * text layer on top so its text can be selected and copied.
 */
export default function PdfPage({ doc, number, scale, guess, root, textLayer: TextLayer }: Props) {
  const boxRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const textRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState<{ width: number; height: number } | null>(null);
  const [near, setNear] = useState(false);

  useEffect(() => {
    const box = boxRef.current;
    if (!box || !root) return undefined;
    const observer = new IntersectionObserver(([entry]) => setNear(entry.isIntersecting), { root, rootMargin: AHEAD });
    observer.observe(box);
    return () => observer.disconnect();
  }, [root]);

  useEffect(() => {
    const canvas = canvasRef.current;
    const text = textRef.current;
    if (!near || !canvas || !text) {
      // Far away: give the pixels back.
      if (canvas) { canvas.width = 0; canvas.height = 0; }
      if (text) text.replaceChildren();
      return undefined;
    }
    let cancelled = false;
    let renderTask: { cancel: () => void } | null = null;
    let layer: { cancel: () => void } | null = null;
    doc.getPage(number).then((page) => {
      if (cancelled) return;
      const base = page.getViewport({ scale: 1 });
      setSize({ width: base.width, height: base.height });
      const viewport = page.getViewport({ scale });
      const ratio = window.devicePixelRatio || 1;
      canvas.width = Math.floor(viewport.width * ratio);
      canvas.height = Math.floor(viewport.height * ratio);
      const task = page.render({
        canvas,
        viewport,
        transform: ratio !== 1 ? [ratio, 0, 0, ratio, 0, 0] : undefined,
      });
      renderTask = task;
      task.promise.catch(() => { /* cancelled by a zoom or a scroll away */ });
      text.replaceChildren();
      const words = new TextLayer({ textContentSource: page.streamTextContent(), container: text, viewport });
      layer = words;
      words.render().catch(() => { /* the picture is what matters */ });
    }).catch(() => { /* the document was closed */ });
    return () => { cancelled = true; renderTask?.cancel(); layer?.cancel(); };
  }, [near, doc, number, scale, TextLayer]);

  const shape = size || guess;
  return (
    <div ref={boxRef} className="pdf-page" data-page={number}
      style={{ width: shape.width * scale, height: shape.height * scale, '--total-scale-factor': scale } as React.CSSProperties}>
      <canvas ref={canvasRef} style={{ width: shape.width * scale, height: shape.height * scale }} aria-hidden="true" />
      <div ref={textRef} className="textLayer" />
    </div>
  );
}
