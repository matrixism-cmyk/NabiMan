import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { I18nProvider } from '../../../../i18n';
import { fetchWithRefresh } from '../../../../shared/api/useApi';
import OfficePreview from './OfficePreview';
import { previewOf } from './previewKinds';

jest.mock('../../../../shared/api/useApi', () => ({ fetchWithRefresh: jest.fn() }));
jest.mock('./PdfPreview', () => ({ __esModule: true, default: ({ blob }: { blob: Blob }) => <div data-testid="pdf">{blob.type}:{blob.size}</div> }));

const fetchMock = fetchWithRefresh as jest.Mock;
const view = (allowed = true) => render(<I18nProvider><OfficePreview blob={new Blob(['PK…'])} name="보고서 최종.docx" allowed={allowed} onInfo={jest.fn()} /></I18nProvider>);
const response = (status: number, type: string, body: BodyInit) => ({
  ok: status === 200, status, headers: { get: (k: string) => (k === 'Content-Type' ? type : null) },
  arrayBuffer: () => new Response(body).arrayBuffer(), json: () => new Response(body).json(),
});

beforeEach(() => { localStorage.setItem('nabiman_lang', 'ko'); fetchMock.mockReset(); });

it('Word, Excel and PowerPoint files are previewed as documents, not archives', () => {
  for (const name of ['a.docx', 'b.DOC', 'c.xlsx', 'd.xls', 'e.pptx', 'f.ppt', 'g.odt', 'h.rtf', '공문.hwp', '양식.HWPX']) expect(previewOf(name)?.kind).toBe('office');
  expect(previewOf('a.zip')?.kind).toBe('zip');
  expect(previewOf('a.pdf')?.kind).toBe('pdf');
  expect(previewOf('a.csv')?.kind).toBe('text');
});

it('sends the file and shows the PDF that comes back', async () => {
  fetchMock.mockResolvedValue(response(200, 'application/pdf', '%PDF-1.7 fake'));
  view();
  expect(screen.getByText('문서를 PDF로 바꾸는 중…')).toBeInTheDocument();
  expect(await screen.findByTestId('pdf')).toHaveTextContent('application/pdf:13');
  const [url, init] = fetchMock.mock.calls[0];
  expect(url).toBe('/api/preview/office?name=%EB%B3%B4%EA%B3%A0%EC%84%9C%20%EC%B5%9C%EC%A2%85.docx');
  expect(init.method).toBe('POST');
});

it('says why when the server cannot convert it', async () => {
  fetchMock.mockResolvedValue(response(422, 'application/json', JSON.stringify({ success: false, message: '변환 시간이 초과되었습니다' })));
  view();
  expect(await screen.findByRole('alert')).toHaveTextContent('변환 시간이 초과되었습니다');
});

it('on a share link it does not ask the server', async () => {
  view(false);
  await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('로그인한 계정'));
  expect(fetchMock).not.toHaveBeenCalled();
});
