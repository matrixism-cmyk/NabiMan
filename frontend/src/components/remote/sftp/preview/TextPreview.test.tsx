import React from 'react';
import { TextDecoder, TextEncoder } from 'util';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../../../i18n';
import { downloadFile, SftpEntry } from '../sftpApi';
import PreviewDialog from './PreviewDialog';

// jsdom has neither; the browser has both.
Object.assign(global, { TextDecoder, TextEncoder });
if (!Blob.prototype.arrayBuffer) {
  Blob.prototype.arrayBuffer = function arrayBuffer(this: Blob) {
    return new Promise<ArrayBuffer>((resolve) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as ArrayBuffer);
      reader.readAsArrayBuffer(this);
    });
  };
}

jest.mock('../sftpApi', () => ({ ...jest.requireActual('../sftpApi'), downloadFile: jest.fn() }));
const download = downloadFile as jest.Mock;
const source = { base: '/api/terminal/sftp/srv', key: 'srv', writable: true };
const file = (name: string, size = 100): SftpEntry => ({ name, kind: 'file', to_dir: false, size, mtime: null, mode: 0o644 });
const serve = (bytes: Uint8Array | string) => download.mockImplementation(() => ({
  promise: Promise.resolve({ ok: true, blob: new Blob([bytes]) }), abort: jest.fn(),
}));

beforeEach(() => {
  localStorage.setItem('nabiman_lang', 'ko');
  localStorage.removeItem('nabiman_preview_wrap');
  download.mockReset();
});

const ui = (name: string) => render(<I18nProvider><PreviewDialog source={source} dir="/srv" files={[file(name)]} start={name}
  onDownload={jest.fn()} onClose={jest.fn()} /></I18nProvider>);

test('source code gets line numbers and colour', async () => {
  serve('// greet\nconst who: string = "world";\nconsole.log(`hi ${who}`);\n');
  ui('hello.ts');
  await waitFor(() => expect(document.querySelectorAll('.tp-row')).toHaveLength(3));
  expect(document.querySelector('.tp-row .tp-no')).toHaveTextContent('1');
  await waitFor(() => expect(document.querySelector('.hljs-keyword')).toHaveTextContent('const'));
  expect(document.querySelector('.hljs-comment')).toHaveTextContent('// greet');
  expect(screen.getByText(/3줄 · UTF-8 · typescript/)).toBeInTheDocument();
});

test('a legacy Korean file is read as EUC-KR, and the encoding can be chosen', async () => {
  serve(new Uint8Array([0xc7, 0xd1, 0xb1, 0xdb, 0x0a]));
  ui('memo.txt');
  expect(await screen.findByText('한글')).toBeInTheDocument();
  await waitFor(() => expect(document.querySelector('.preview-facts')).toHaveTextContent('1줄 · EUC-KR'));
  fireEvent.change(screen.getByRole('combobox', { name: '문자 인코딩' }), { target: { value: 'iso-8859-1' } });
  await waitFor(() => expect(screen.queryByText('한글')).not.toBeInTheDocument());
});

test('a binary file says so, and can still be shown', async () => {
  serve(new Uint8Array([0x7f, 0x45, 0x4c, 0x46, 0x00, 0x41]));
  ui('LICENSE');
  expect(await screen.findByText(/바이너리 파일/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '그래도 텍스트로 보기' }));
  await waitFor(() => expect(document.querySelectorAll('.tp-row')).toHaveLength(1));
});

test('W wraps long lines and is remembered; Copy all copies the text', async () => {
  const writeText = jest.fn(() => Promise.resolve());
  Object.assign(navigator, { clipboard: { writeText } });
  serve('a long line\nsecond\n');
  ui('notes.log');
  await waitFor(() => expect(document.querySelector('.text-preview')).toBeTruthy());
  fireEvent.keyDown(screen.getByRole('dialog'), { key: 'w' });
  expect(document.querySelector('.text-preview.is-wrap')).toBeTruthy();
  expect(localStorage.getItem('nabiman_preview_wrap')).toBe('1');
  fireEvent.click(screen.getByRole('button', { name: '전체 복사' }));
  await waitFor(() => expect(writeText).toHaveBeenCalledWith('a long line\nsecond\n'));
  expect(await screen.findByRole('button', { name: '복사됨' })).toBeInTheDocument();
});
