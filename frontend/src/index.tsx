import React from 'react';
import ReactDOM from 'react-dom/client';
import './index.css';
import App from './App';
import { I18nProvider } from './i18n';
import { TerminalSettingsProvider } from './components/terminal/settings';
import { TerminalWindowsProvider } from './components/terminal/TerminalWindows';
import SharePage from './components/terminal/SharePage';
import reportWebVitals from './reportWebVitals';

const root = ReactDOM.createRoot(
  document.getElementById('root') as HTMLElement
);
// /share/<token> opens one shared terminal for someone without an account.
const shareToken = window.location.pathname.startsWith('/share/')
  ? window.location.pathname.slice('/share/'.length).replace(/\/$/, '')
  : '';

root.render(
  <React.StrictMode>
    <I18nProvider>
      {shareToken ? (
        <SharePage token={shareToken} />
      ) : (
        <TerminalSettingsProvider>
          <TerminalWindowsProvider>
            <App />
          </TerminalWindowsProvider>
        </TerminalSettingsProvider>
      )}
    </I18nProvider>
  </React.StrictMode>
);

// If you want to start measuring performance in your app, pass a function
// to log results (for example: reportWebVitals(console.log))
// or send to an analytics endpoint. Learn more: https://bit.ly/CRA-vitals
reportWebVitals();
