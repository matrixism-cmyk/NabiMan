import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../../../i18n';
import { downloadFile, SftpEntry } from '../sftpApi';
import PreviewDialog from './PreviewDialog';
import { previewOf } from './previewKinds';

jest.mock('../sftpApi', () => ({ ...jest.requireActual('../sftpApi'), downloadFile: jest.fn() }));
const download = downloadFile as jest.Mock;
const source = { base: '/api/terminal/sftp/srv', key: 'srv', writable: true };
const file = (name: string, size = 1000): SftpEntry => ({ name, kind: 'file', to_dir: false, size, mtime: null, mode: 0o644 });

beforeEach(() => {
  localStorage.setItem('nabiman_lang', 'ko');
  download.mockReset();
  download.mockImplementation(() => ({ promise: Promise.resolve({ ok: true, blob: new Blob(['x']) }), abort: jest.fn() }));
  URL.createObjectURL = jest.fn(() => 'blob:preview');
  URL.revokeObjectURL = jest.fn();
});

const ui = (files: SftpEntry[], start: string, onDownload = jest.fn()) => render(
  <I18nProvider><PreviewDialog source={source} dir="/srv/pics" files={files} start={start} onDownload={onDownload} onClose={jest.fn()} /></I18nProvider>);

test('image types are recognised by name, others are not (yet)', () => {
  expect(previewOf('Photo.JPG')).toEqual(expect.objectContaining({ kind: 'image', mime: 'image/jpeg' }));
  expect(previewOf('icon.svg')?.mime).toBe('image/svg+xml');
  expect(previewOf('notes.txt')).toBeNull();
  expect(previewOf('.png')).toBeNull();
});

test('an image is fetched from its folder and shown', async () => {
  ui([file('a.png'), file('b.jpg')], 'b.jpg');
  expect(await screen.findByRole('img', { name: 'b.jpg' })).toHaveAttribute('src', 'blob:preview');
  expect(download).toHaveBeenCalledWith(source, '/srv/pics/b.jpg', expect.any(Function));
  expect(screen.getByText(/2 \/ 2/)).toBeInTheDocument();
});

test('arrows step through the folder\'s images, wrapping around', async () => {
  ui([file('a.png'), file('b.jpg'), file('c.gif')], 'c.gif');
  await screen.findByRole('img', { name: 'c.gif' });
  fireEvent.keyDown(screen.getByRole('dialog'), { key: 'ArrowRight' });
  expect(await screen.findByRole('img', { name: 'a.png' })).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '이전 파일' }));
  expect(await screen.findByRole('img', { name: 'c.gif' })).toBeInTheDocument();
});

test('a picture too large is not fetched, and can still be downloaded', async () => {
  const onDownload = jest.fn();
  ui([file('huge.png', 80 * 1024 * 1024)], 'huge.png', onDownload);
  expect(screen.getByText(/미리보기로는 열지 않습니다/)).toBeInTheDocument();
  expect(download).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: /내려받기/ }));
  await waitFor(() => expect(onDownload).toHaveBeenCalledWith(expect.objectContaining({ name: 'huge.png' })));
});

test('fit and actual size toggle by button or F', async () => {
  ui([file('a.png')], 'a.png');
  await screen.findByRole('img', { name: 'a.png' });
  const toggle = screen.getByRole('button', { name: '원래 크기' });
  fireEvent.click(toggle);
  expect(document.querySelector('.preview-image.is-actual')).toBeTruthy();
  fireEvent.keyDown(screen.getByRole('dialog'), { key: 'f' });
  expect(document.querySelector('.preview-image.is-fit')).toBeTruthy();
});
