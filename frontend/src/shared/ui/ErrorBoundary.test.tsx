import React, { useState } from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { ErrorBoundary } from './ErrorBoundary';
import { redact } from './crashReport';

function Bomb({ message = 'panel exploded' }: { message?: string }): React.ReactElement {
  throw new Error(message);
}

function Shell() {
  const [tab, setTab] = useState('broken');
  return (
    <div>
      <nav>
        <button onClick={() => setTab('broken')}>broken tab</button>
        <button onClick={() => setTab('fine')}>fine tab</button>
      </nav>
      <main>
        <ErrorBoundary region={`panel:${tab}`} resetKey={tab}>
          {tab === 'broken' ? <Bomb /> : <p>fine panel</p>}
        </ErrorBoundary>
      </main>
    </div>
  );
}

const renderKo = (ui: React.ReactElement) => {
  localStorage.setItem('nabiman_lang', 'ko');
  return render(<I18nProvider>{ui}</I18nProvider>);
};

beforeEach(() => { jest.spyOn(console, 'error').mockImplementation(() => undefined); });
afterEach(() => { (console.error as jest.Mock).mockRestore(); });

test('a throwing child shows the translated fallback instead of a blank page', () => {
  renderKo(<ErrorBoundary variant="app"><Bomb /></ErrorBoundary>);
  expect(screen.getByRole('alert')).toHaveTextContent('화면을 표시하다 문제가 생겼습니다');
  expect(screen.getByText(/panel exploded/)).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '새로고침' })).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '오류 정보 복사' })).toBeInTheDocument();
});

test('the shell around a throwing panel keeps working, and another tab recovers', () => {
  renderKo(<Shell />);
  expect(screen.getByRole('alert')).toHaveTextContent('이 부분을 표시하지 못했습니다');
  fireEvent.click(screen.getByText('fine tab'));
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  expect(screen.getByText('fine panel')).toBeInTheDocument();
});

test('try again re-renders the region', () => {
  let fail = true;
  function Flaky() { if (fail) throw new Error('once'); return <p>recovered</p>; }
  renderKo(<ErrorBoundary><Flaky /></ErrorBoundary>);
  fail = false;
  fireEvent.click(screen.getByRole('button', { name: '다시 시도' }));
  expect(screen.getByText('recovered')).toBeInTheDocument();
});

test('copy details carries the error, the component stack and the version, never a token', async () => {
  sessionStorage.setItem('nabiman_token', 'eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhZG1pbiJ9.c2lnbmF0dXJl');
  const writeText = jest.fn(() => Promise.resolve());
  Object.assign(navigator, { clipboard: { writeText } });
  jest.spyOn(window, 'fetch').mockResolvedValue(new Response(JSON.stringify({
    success: true, data: { package_version: '1.2.3', git_commit: 'abc1234' }, message: '',
  })));
  renderKo(<ErrorBoundary region="panel:ssl"><Bomb message="bad Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhZG1pbiJ9.c2lnbmF0dXJl" /></ErrorBoundary>);
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '오류 정보 복사' })); });
  await waitFor(() => expect(writeText).toHaveBeenCalled());
  const report = (writeText.mock.calls[0] as unknown as [string])[0];
  expect(report).toContain('Region: panel:ssl');
  expect(report).toContain('Version: 1.2.3 @ abc1234');
  expect(report).toContain('Component stack:');
  expect(report).toContain('Bomb');
  expect(report).not.toContain('c2lnbmF0dXJl');
  expect(screen.getByText('복사했습니다')).toBeInTheDocument();
  sessionStorage.clear();
});

test('redact blanks share links, API keys and query secrets', () => {
  expect(redact('/share/abcdef123/x nbm_SECRET ?ticket=zzz&x=1')).toBe('/share/[link]/x nbm_[key] ?ticket=[redacted]&x=1');
});
