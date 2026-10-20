/**
 * What the file panel can show in its preview, by file name. Each kind has
 * its own renderer in PreviewDialog; adding a format is adding a row here
 * and, for a new kind, a renderer.
 */
export type PreviewKind = 'image';

export interface PreviewSpec {
  kind: PreviewKind;
  /** The type the bytes are labelled with, so the browser decodes them right. */
  mime: string;
  /** Larger files are not fetched for a preview; they can still be downloaded. */
  maxBytes: number;
}

const MB = 1024 * 1024;

const IMAGES: Record<string, string> = {
  png: 'image/png', apng: 'image/apng',
  jpg: 'image/jpeg', jpeg: 'image/jpeg', jpe: 'image/jpeg', jfif: 'image/jpeg',
  gif: 'image/gif', webp: 'image/webp', avif: 'image/avif',
  bmp: 'image/bmp', ico: 'image/x-icon', cur: 'image/x-icon',
  // Shown through <img>, where an SVG's scripts never run.
  svg: 'image/svg+xml',
};

export const extensionOf = (name: string) => {
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : '';
};

export function previewOf(name: string): PreviewSpec | null {
  const ext = extensionOf(name);
  if (IMAGES[ext]) return { kind: 'image', mime: IMAGES[ext], maxBytes: 50 * MB };
  return null;
}
