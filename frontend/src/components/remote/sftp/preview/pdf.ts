/**
 * pdf.js, loaded on demand (`import('./pdf')`) so the main bundle does not
 * carry it. Parsing runs in its own worker, served from this site; nothing in
 * a PDF is ever run as script — pdf.js only draws it.
 */
import { getDocument, GlobalWorkerOptions, PasswordResponses, TextLayer } from 'pdfjs-dist';

// The bundler emits the worker as a file of its own and gives its URL.
GlobalWorkerOptions.workerSrc = new URL('pdfjs-dist/build/pdf.worker.min.mjs', import.meta.url).toString();

export type { PDFDocumentProxy, PDFPageProxy } from 'pdfjs-dist';

/** Opens a PDF from its bytes. `askPassword` is called while one is needed
 *  (`retry` is true after a wrong one) and resolves with what was typed. */
export function openPdf(data: Uint8Array, askPassword: (retry: boolean) => Promise<string>) {
  const task = getDocument({
    data,
    // No fonts, CMaps or other files fetched from anywhere: what is not
    // embedded is drawn with a substitute.
    useSystemFonts: true,
    stopAtErrors: false,
  });
  task.onPassword = (update: (password: string) => void, reason: number) => {
    askPassword(reason === PasswordResponses.INCORRECT_PASSWORD).then(update, () => task.destroy());
  };
  return task;
}

export { TextLayer };
