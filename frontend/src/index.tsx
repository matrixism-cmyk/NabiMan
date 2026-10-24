import React from 'react';
import ReactDOM from 'react-dom/client';
import './shared/styles';
import App from './App';
import { I18nProvider } from './i18n';
import SharePage from './components/terminal/SharePage';
import ConsoleSharePage from './components/remote/ConsoleSharePage';
import { ConfirmHost } from './shared/ui/ConfirmDialog';
import reportWebVitals from './reportWebVitals';

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
      {consoleShareToken ? (
        <ConsoleSharePage token={consoleShareToken} />
      ) : shareToken ? (
        <SharePage token={shareToken} />
      ) : (
        <App />
      )}
      <ConfirmHost />
    </I18nProvider>
  </React.StrictMode>
);

// If you want to start measuring performance in your app, pass a function
// to log results (for example: reportWebVitals(console.log))
// or send to an analytics endpoint. Learn more: https://bit.ly/CRA-vitals
reportWebVitals();
