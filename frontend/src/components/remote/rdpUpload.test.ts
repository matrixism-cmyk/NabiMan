import Guacamole from '../../vendor/guacamole-common.js';
import { sendWhenOpen } from './rdpUpload';

const ok = () => new Guacamole.Status(Guacamole.Status.Code.SUCCESS, 'OK');

/** A stream as guacd answers it: every blob acknowledged a moment later. */
function fakeStream() {
  const blobs: string[] = [];
  const stream: any = { sendEnd: jest.fn(), onack: null };
  stream.sendBlob = (b64: string) => { blobs.push(b64); setTimeout(() => stream.onack?.(ok()), 0); };
  return { stream, blobs };
}

const decode = (blobs: string[]) => Buffer.concat(blobs.map((b) => Buffer.from(b, 'base64')));
const settle = () => new Promise((r) => setTimeout(r, 0));

test('nothing is sent before guacd opens the stream, then every byte arrives in order and the stream ends', async () => {
  const bytes = new Uint8Array(6048 * 20 + 123).map((_, i) => (i * 7 + (i >> 8)) & 0xff);
  const { stream, blobs } = fakeStream();
  const done = jest.fn();
  sendWhenOpen(stream, new Blob([bytes]), { progress: jest.fn(), done, failed: jest.fn() });
  await settle();
  expect(blobs).toHaveLength(0);
  stream.onack(ok()); // guacd: stream open
  for (let i = 0; i < 200 && !done.mock.calls.length; i++) await settle();
  expect(done).toHaveBeenCalledTimes(1);
  expect(stream.sendEnd).toHaveBeenCalledTimes(1);
  expect(Buffer.compare(decode(blobs), Buffer.from(bytes))).toBe(0);
});

test('a refused stream fails without sending', async () => {
  const { stream, blobs } = fakeStream();
  const failed = jest.fn();
  sendWhenOpen(stream, new Blob([new Uint8Array(10)]), { progress: jest.fn(), done: jest.fn(), failed });
  stream.onack(new Guacamole.Status(Guacamole.Status.Code.RESOURCE_NOT_FOUND, 'no'));
  await settle();
  expect(failed).toHaveBeenCalled();
  expect(blobs).toHaveLength(0);
});
