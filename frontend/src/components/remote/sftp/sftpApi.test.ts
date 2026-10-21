import { accountSource, PART_SIZE, uploadFile } from './sftpApi';
import { apiRequest } from '../../../shared/api/useApi';

jest.mock('../../../shared/api/useApi', () => ({ apiRequest: jest.fn() }));
const request = apiRequest as jest.Mock;

/** Answers each upload request with the next scripted reply. */
type Reply = { status: number; body?: object } | 'network';
let replies: Reply[] = [];
const sent: { url: string; size: number }[] = [];

class FakeXhr {
  status = 0;
  responseText = '';
  upload: { onprogress?: (e: ProgressEvent) => void } = {};
  onload?: () => void;
  onerror?: () => void;
  onabort?: () => void;
  private url = '';
  open(_method: string, url: string) { this.url = url; }
  setRequestHeader() {}
  abort() { setTimeout(() => this.onabort?.(), 0); }
  send(body: Blob) {
    sent.push({ url: this.url, size: body.size });
    const reply = replies.shift() || { status: 200, body: { success: true, data: {} } };
    setTimeout(() => {
      if (reply === 'network') { this.onerror?.(); return; }
      this.status = reply.status;
      this.responseText = JSON.stringify(reply.body || {});
      this.onload?.();
    }, 0);
  }
}

const offsets = () => sent.map((s) => Number(new URL(s.url, 'http://x').searchParams.get('offset')));
const blob = (size: number) => ({ size, slice: (a: number, b: number) => ({ size: b - a }) }) as unknown as Blob;

beforeEach(() => {
  jest.useFakeTimers();
  replies = [];
  sent.length = 0;
  (global as unknown as { XMLHttpRequest: unknown }).XMLHttpRequest = FakeXhr;
  request.mockResolvedValue({ success: true, data: { size: 0 }, message: '' });
});
afterEach(() => jest.useRealTimers());

async function run(size: number) {
  const transfer = uploadFile(accountSource('srv_1'), '/data', 'big.iso', blob(size), false, () => {});
  // Present at runtime; this @types/jest predates it.
  await (jest as unknown as { runAllTimersAsync: () => Promise<void> }).runAllTimersAsync();
  return transfer.promise;
}

it('sends a small file in one request', async () => {
  await expect(run(1024)).resolves.toMatchObject({ ok: true });
  expect(sent).toHaveLength(1);
  expect(sent[0].url).toContain('/upload?');
});

it('sends a large file in pieces, then names it', async () => {
  const size = PART_SIZE * 2 + 10;
  await expect(run(size)).resolves.toMatchObject({ ok: true });
  expect(offsets()).toEqual([0, PART_SIZE, PART_SIZE * 2]);
  expect(sent.map((s) => s.size)).toEqual([PART_SIZE, PART_SIZE, 10]);
  expect(request).toHaveBeenCalledWith(expect.stringContaining(`/upload-finish?`), { method: 'POST' });
  expect(request.mock.calls[0][0]).toContain(`size=${size}`);
});

it('resends a piece that failed', async () => {
  replies = [{ status: 200, body: { success: true } }, 'network', { status: 502, body: { success: false } }];
  await expect(run(PART_SIZE * 2)).resolves.toMatchObject({ ok: true });
  expect(offsets()).toEqual([0, PART_SIZE, PART_SIZE, PART_SIZE]);
});

it('continues from where the server got to', async () => {
  // The second piece arrived but its answer was lost; the retry is told so.
  replies = [{ status: 200 }, 'network', { status: 409, body: { success: false, data: { size: PART_SIZE * 2 } } }];
  await expect(run(PART_SIZE * 3)).resolves.toMatchObject({ ok: true });
  expect(offsets()).toEqual([0, PART_SIZE, PART_SIZE, PART_SIZE * 2]);
});

it('stops at an existing file before sending anything else', async () => {
  replies = [{ status: 409, body: { success: false, data: { exists: true } } }];
  await expect(run(PART_SIZE * 3)).resolves.toMatchObject({ ok: false, exists: true });
  expect(sent).toHaveLength(1);
});

it('gives up after repeated failures and removes the part file', async () => {
  replies = Array(10).fill('network');
  await expect(run(PART_SIZE * 2)).resolves.toMatchObject({ ok: false });
  expect(sent).toHaveLength(4);
  expect(request).toHaveBeenCalledWith(expect.stringContaining('/upload-abort?'), { method: 'POST' });
});
