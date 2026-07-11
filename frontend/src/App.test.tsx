import React from 'react';
import { render, screen } from '@testing-library/react';
import { I18nProvider } from './i18n';
import App from './App';

// Smoke test: with no session token the app renders the login screen
// (username textbox + action buttons) without throwing.
test('renders the login screen when logged out', () => {
  sessionStorage.clear();
  render(
    <I18nProvider>
      <App />
    </I18nProvider>,
  );
  expect(screen.getAllByRole('textbox').length).toBeGreaterThan(0);
  expect(screen.getAllByRole('button').length).toBeGreaterThan(0);
});
