import React from 'react';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '../../../i18n';
import PropertiesDialog from './PropertiesDialog';
import { accountSource } from './sftpApi';
import * as usageApi from './sftpUsage';

jest.mock('./sftpUsage', () => ({ ...jest.requireActual('./sftpUsage'), usage: jest.fn(), checksum: jest.fn() }));
const usage = usageApi.usage as jest.Mock;
const checksum = usageApi.checksum as jest.Mock;

const folder: usageApi.UsageItem = { name: 'logs', path: '/home/me/logs', kind: 'dir', to_dir: false, size: 4096, mtime: 1_700_000_000, mode: 0o755, owner: 'me', group: 'staff' };
const file: usageApi.UsageItem = { name: 'app.tar', path: '/home/me/app.tar', kind: 'file', to_dir: false, size: 2_500_000, mtime: 1_700_000_000, mode: 0o644, uid: 1000, gid: 1000 };
const count = (dirs: number, files: number, bytes = 0, unreadable = 0) => ({ dirs, files, links: 0, other: 0, bytes, unreadable });

function show(paths: string[], onAttrs?: () => void) {
  return render(
    <I18nProvider>
      <PropertiesDialog source={accountSource('srv_1')} paths={paths} onAttrs={onAttrs} onClose={() => {}} />
    </I18nProvider>,
  );
}

/** Hands the dialog the lines of an answer one at a time; the answer ends when `end` is called. */
function answer() {
  let push: (l: usageApi.UsageLine) => void = () => {};
  let end: () => void = () => {};
  let signal: AbortSignal | null = null;
  usage.mockImplementation((_s, _p, onLine, sig) => {
    push = onLine; signal = sig;
    return new Promise<string>((r) => { end = () => r(''); });
  });
  return { push: (l: usageApi.UsageLine) => act(() => push(l)), end: () => act(async () => end()), signal: () => signal };
}

beforeEach(() => { localStorage.setItem('nabiman_lang', 'ko'); usage.mockReset(); checksum.mockReset(); });

test('a folder shows what is in it, then the running count, then where the count fell short', async () => {
  const a = answer();
  show(['/home/me/logs']);
  a.push({ items: [folder], inside: { dirs: 2, files: 5, links: 0, other: 0, hidden: 3 } });
  expect(screen.getByText('/home/me/logs')).toBeInTheDocument();
  expect(screen.getByText('rwxr-xr-x')).toBeInTheDocument();
  expect(screen.getByText('me : staff')).toBeInTheDocument();
  expect(screen.getByText(/폴더 2개 · 파일 5개/)).toBeInTheDocument();
  expect(screen.getByText(/숨김 3개/)).toBeInTheDocument();

  a.push({ count: count(10, 1200, 3_000_000) });
  expect(screen.getByText(/집계 중… 1,210개/)).toBeInTheDocument();
  a.push({ count: count(12, 200_000, 9_000_000, 2), done: true, capped: 'entries', how: 'find' });
  await a.end();
  expect(screen.getByText(/폴더 12개 · 파일 200,000개 이상/)).toBeInTheDocument();
  expect(screen.getByText(/항목이 너무 많아/)).toBeInTheDocument();
  expect(screen.getByText(/읽을 수 없는 폴더 2개/)).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '중지' })).not.toBeInTheDocument();
});

test('stopping the count closes the request and keeps what was counted', async () => {
  const a = answer();
  show(['/home/me/logs']);
  a.push({ items: [folder], inside: null });
  a.push({ count: count(4, 30) });
  fireEvent.click(screen.getByRole('button', { name: '중지' }));
  expect(a.signal()?.aborted).toBe(true);
  expect(screen.getByText(/파일 30개 이상/)).toBeInTheDocument();
  expect(screen.getByText(/중지했습니다/)).toBeInTheDocument();
});

test('a file shows its exact size and works out its SHA-256 only when asked', async () => {
  const a = answer();
  show(['/home/me/app.tar']);
  a.push({ items: [file], inside: null });
  a.push({ count: count(0, 0), done: true, capped: null, how: 'none' });
  await a.end();
  expect(screen.getByText(/2,500,000 B/)).toBeInTheDocument();
  expect(screen.getByText('1000 : 1000')).toBeInTheDocument();
  expect(checksum).not.toHaveBeenCalled();
  checksum.mockImplementation((_s, path, onLine) => {
    onLine({ read: 2_500_000, size: 2_500_000, sha256: `${path.length}`.padEnd(64, 'a') });
    return Promise.resolve('');
  });
  fireEvent.click(screen.getByRole('button', { name: '계산' }));
  expect(await screen.findByText('16'.padEnd(64, 'a'))).toBeInTheDocument();
});

test('several entries are summed up, and the permissions button hands them over', async () => {
  const a = answer();
  const onAttrs = jest.fn();
  show(['/home/me/logs', '/home/me/app.tar'], onAttrs);
  a.push({ items: [folder, file], inside: null });
  expect(screen.getByText(/2개 선택됨 — 폴더 1개 · 파일 1개/)).toBeInTheDocument();
  a.push({ count: count(3, 9, 2_600_000), done: true, capped: null, how: 'local' });
  await a.end();
  expect(screen.getByText('선택 항목 전체')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: /변경/ }));
  expect(onAttrs).toHaveBeenCalledWith([folder, file]);
});
