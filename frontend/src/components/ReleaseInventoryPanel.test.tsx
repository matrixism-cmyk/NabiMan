import React from 'react';
import { render, screen } from '@testing-library/react';
import { I18nProvider } from '../i18n';
import ReleaseInventoryPanel from './ReleaseInventoryPanel';
const previousFetch = global.fetch;
beforeEach(() => { localStorage.setItem('nabiman_lang', 'en'); });
afterEach(() => { global.fetch = previousFetch; });
function inventory() { return { schema_version: 1, kind: 'unsigned-packaging-inventory', components: 5, artifacts: { 'nabiman-server': { sha256: 'a'.repeat(64) } } }; }
it('compares actual runtime identity without treating unsigned inventory as publisher proof', async () => {
  global.fetch = jest.fn().mockResolvedValue({ ok: true, json: async () => inventory() });
  const view = render(<I18nProvider><ReleaseInventoryPanel runtimeSha={'a'.repeat(64)} /></I18nProvider>);
  expect(await screen.findByRole('status')).toHaveTextContent('matches the running executable');
  expect(screen.getByText(/It does not prove binary composition/)).toBeInTheDocument();
  view.rerender(<I18nProvider><ReleaseInventoryPanel runtimeSha={'b'.repeat(64)} /></I18nProvider>);
  expect(screen.getByRole('alert')).toHaveTextContent('hashes differ');
  view.rerender(<I18nProvider><ReleaseInventoryPanel /></I18nProvider>);
  expect(screen.getByRole('status')).toHaveTextContent('requires runtime identity');
});
it('does not show download links or a success claim for malformed metadata', async () => {
  global.fetch = jest.fn().mockResolvedValue({ ok: true, json: async () => ({ signed: true }) });
  render(<I18nProvider><ReleaseInventoryPanel runtimeSha={'a'.repeat(64)} /></I18nProvider>);
  expect(await screen.findByRole('alert')).toHaveTextContent('unavailable');
  expect(screen.queryByRole('link')).not.toBeInTheDocument();
});
