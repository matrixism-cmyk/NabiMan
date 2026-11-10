import React from 'react';
import ReactDOM from 'react-dom/client';
import './shared/styles';
import App from './App';
import { I18nProvider } from './i18n';
import SharePage from './components/terminal/SharePage';
import ConsoleSharePage from './components/remote/ConsoleSharePage';
import { ConfirmHost } from './shared/ui/ConfirmDialog';
import { ErrorBoundary } from './shared/ui/ErrorBoundary';
import GlobalErrorNotices from './shared/ui/GlobalErrorNotices';
import { installGlobalErrorHandlers } from './shared/ui/globalErrors';
import reportWebVitals from './reportWebVitals';

// Errors outside React's reach (handlers, timers, promises) become a notice.
installGlobalErrorHandlers();

const root = ReactDOM.createRoot(
  document.getElementById('root') as HTMLElement
);
// /share/<token> opens one shared terminal for someone without an account.
const shareToken = window.location.pathname.startsWith('/share/')
  ? window.location.pathname.slice('/share/'.length).replace(/\/$/, '')
  : '';
// /cshare/<token> opens one shared VM console for someone without an account.
const consoleShareToken = window.location.pathname.startsWith('/cshare/')
  ? window.location.pathname.slice('/cshare/'.length).replace(/\/$/, '')
  : '';

root.render(
  <React.StrictMode>
    <I18nProvider>
      <ErrorBoundary variant="app" region="app">
        {consoleShareToken ? (
          <ConsoleSharePage token={consoleShareToken} />
        ) : shareToken ? (
          <SharePage token={shareToken} />
        ) : (
          <App />
        )}
      </ErrorBoundary>
      <ConfirmHost />
      <GlobalErrorNotices />
    </I18nProvider>
  </React.StrictMode>
);

// Web Vitals stay off unless a handler is passed (e.g. reportWebVitals(console.log)).
reportWebVitals();
