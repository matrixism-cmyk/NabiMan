import React from 'react';
import { TextDecoder, TextEncoder } from 'util';
import { render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../../../i18n';
import { readRange, SftpEntry, streamLink } from '../sftpApi';
import PreviewDialog from './PreviewDialog';

Object.assign(global, { TextDecoder, TextEncoder });
jest.mock('../sftpApi', () => ({ ...jest.requireActual('../sftpApi'), streamLink: jest.fn(), readRange: jest.fn() }));
const link = streamLink as jest.Mock;
const range = readRange as jest.Mock;
const source = { base: '/api/terminal/sftp/srv', key: 'srv', writable: true };
const file = (name: string, size: number): SftpEntry => ({ name, kind: 'file', to_dir: false, size, mtime: null, mode: 0o644 });

beforeEach(() => {
  localStorage.setItem('nabiman_lang', 'ko');
  link.mockReset();
  range.mockReset();
  link.mockImplementation((_s, path: string) => Promise.resolve(`/stream?path=${path}`));
});

const ui = (entry: SftpEntry) => render(<I18nProvider><PreviewDialog source={source} dir="/d" files={[entry]} start={entry.name}
  onDownload={jest.fn()} onClose={jest.fn()} /></I18nProvider>);

test('a video plays from its stream link, nothing fetched first', async () => {
  ui(file('clip.mp4', 3e9));
  await waitFor(() => expect(document.querySelector('video')).toHaveAttribute('src', '/stream?path=/d/clip.mp4'));
  expect(range).not.toHaveBeenCalled();
});

test('a song gets a player with its name', async () => {
  ui(file('song.flac', 4e7));
  await waitFor(() => expect(document.querySelector('audio')).toHaveAttribute('src', '/stream?path=/d/song.flac'));
  expect(screen.getAllByText('song.flac').length).toBeGreaterThan(1);
});

test('a ZIP is listed from the ranges at its end', async () => {
  // One entry "a.txt", directory at 0, end record after it.
  const dir = [0x50, 0x4b, 0x01, 0x02, 20, 0, 20, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 9, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, ...Array.from(new TextEncoder().encode('a.txt'))];
  const end = [0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0, 1, 0, 1, 0, dir.length, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  const whole = new Uint8Array([...dir, ...end]);
  range.mockImplementation((_u: string, a: number, b: number) => Promise.resolve(whole.slice(a, b + 1)));
  ui(file('pack.zip', whole.length));
  expect(await screen.findByText('a.txt')).toBeInTheDocument();
  expect(screen.getByText(/파일 1개, 풀면 9 B/)).toBeInTheDocument();
});

test('an unknown binary shows what it is and its first bytes in hex', async () => {
  range.mockResolvedValue(new Uint8Array([0x7f, 0x45, 0x4c, 0x46, 2, 1, 1, 0]));
  ui(file('tool', 8));
  await waitFor(() => expect(document.querySelector('.preview-facts')).toHaveTextContent('ELF 실행 파일'));
  expect(document.querySelector('.hex-preview')).toHaveTextContent('7f 45 4c 46');
});
