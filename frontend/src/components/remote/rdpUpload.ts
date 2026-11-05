import Guacamole from '../../vendor/guacamole-common.js';

interface Callbacks {
  progress: (offset: number) => void;
  done: () => void;
  failed: () => void;
}

/**
 * Sends a file down a Guacamole output stream once guacd has opened it.
 *
 * guacd acknowledges the stream itself before any data. A BlobWriter started
 * at once takes that acknowledgement for its first chunk's, so two chunk
 * reads run at the same time on its one FileReader; the second throws and its
 * chunk is skipped — 6048 bytes missing from the middle of the file, a
 * picture cut off part-way. So the writer starts only on that first
 * acknowledgement, and the stream is ended when the last chunk is through.
 */
export function sendWhenOpen(stream: any, file: Blob, cb: Callbacks): void {
  stream.onack = (status: any) => {
    if (status.isError()) { cb.failed(); return; }
    // The writer takes over the stream's acknowledgements from here.
    const writer = new Guacamole.BlobWriter(stream);
    let over = false;
    const finish = (ok: boolean) => {
      if (over) return;
      over = true;
      writer.sendEnd();
      if (ok) cb.done(); else cb.failed();
    };
    writer.onprogress = (_blob: Blob, offset: number) => cb.progress(offset);
    writer.onack = (s: any) => { if (s.isError()) finish(false); };
    writer.oncomplete = () => finish(true);
    writer.onerror = () => finish(false);
    writer.sendBlob(file);
  };
}
