import React from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../../../i18n';
import PdfPreview from './PdfPreview';

// pdf.js itself is not loaded under jsdom: a stand-in document of 3 pages.
const mockRendered: number[] = [];
let mockAsk: ((retry: boolean) => Promise<string>) | null = null;
let mockFailWith: Error | null = null;
const mockPage = (n: number) => ({
  getViewport: ({ scale }: { scale: number }) => ({ width: 600 * scale, height: 800 * scale, scale }),
  render: () => { mockRendered.push(n); return { promise: Promise.resolve(), cancel: jest.fn() }; },
  streamTextContent: () => ({}),
});
jest.mock('./pdf', () => ({
  openPdf: (_data: Uint8Array, askPassword: (retry: boolean) => Promise<string>) => {
    mockAsk = askPassword;
    return {
      promise: mockFailWith ? Promise.reject(mockFailWith) : Promise.resolve({ numPages: 3, getPage: (n: number) => Promise.resolve(mockPage(n)) }),
      destroy: jest.fn(() => Promise.resolve()),
    };
  },
  TextLayer: class { render() { return Promise.resolve(); } cancel() { /* nothing to stop */ } },
}));

beforeAll(() => {
  // Every page counts as near the view; boxes are 800 px wide.
  Object.assign(window, {
    IntersectionObserver: class {
      constructor(private cb: (e: { isIntersecting: boolean }[]) => void) {}
      observe() { this.cb([{ isIntersecting: true }]); }
      disconnect() { /* nothing to stop */ }
    },
    ResizeObserver: class { observe() { /* sized once below */ } disconnect() { /* nothing */ } },
  });
  Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => 840 });
  if (!Blob.prototype.arrayBuffer) {
    Blob.prototype.arrayBuffer = function arrayBuffer() { return Promise.resolve(new ArrayBuffer(4)); };
  }
});

beforeEach(() => { localStorage.setItem('nabiman_lang', 'ko'); mockRendered.length = 0; mockAsk = null; mockFailWith = null; });

const ui = (onInfo = jest.fn()) => render(<I18nProvider><PdfPreview blob={new Blob(['%PDF'])} onInfo={onInfo} /></I18nProvider>);

test('every page gets a place, pages near the view are drawn, and the count is reported', async () => {
  const onInfo = jest.fn();
  ui(onInfo);
  await waitFor(() => expect(document.querySelectorAll('.pdf-page')).toHaveLength(3));
  await waitFor(() => expect(mockRendered.sort()).toEqual([1, 2, 3]));
  expect(onInfo).toHaveBeenCalledWith({ pages: 3 });
  expect(screen.getByText('/ 3')).toBeInTheDocument();
  // Fitted to the width: (840 - 40) / 600.
  expect(screen.getByText('133%')).toBeInTheDocument();
});

test('+, − and 0 zoom by steps and back to the width', async () => {
  ui();
  await waitFor(() => expect(document.querySelectorAll('.pdf-page')).toHaveLength(3));
  const scroll = document.querySelector('.pdf-scroll') as HTMLElement;
  fireEvent.keyDown(scroll, { key: '+' });
  expect(screen.getByText('150%')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '축소 (−)' }));
  expect(screen.getByText('125%')).toBeInTheDocument();
  fireEvent.keyDown(scroll, { key: '0' });
  expect(screen.getByText('133%')).toBeInTheDocument();
  expect((document.querySelector('.pdf-page') as HTMLElement).style.width).toBe('800px');
});

test('a locked PDF asks for its password, again when it is wrong', async () => {
  const { container } = ui();
  await waitFor(() => expect(mockAsk).not.toBeNull());
  let typed: Promise<string> = Promise.resolve('');
  act(() => { typed = mockAsk!(false); });
  fireEvent.change(await screen.findByLabelText('PDF 암호'), { target: { value: 'secret' } });
  fireEvent.submit(container.querySelector('form') as HTMLFormElement);
  await expect(typed).resolves.toBe('secret');
  act(() => { mockAsk!(true); });
  expect(await screen.findByText(/암호가 맞지 않습니다/)).toBeInTheDocument();
});

test('a file that is not a PDF says so', async () => {
  mockFailWith = new Error('Invalid PDF structure');
  ui();
  expect(await screen.findByRole('alert')).toHaveTextContent('PDF를 읽을 수 없습니다');
});
