import React, { useState, useEffect, useMemo, useCallback } from 'react';
import './App.css';
import './components.css';
import LoginScreen from './components/LoginScreen';
import TerminalPanel from './components/TerminalPanel';
import PanelOutlet from './components/PanelOutlet';
import ChangePasswordModal from './components/ChangePasswordModal';
import CommandPalette from './components/CommandPalette';
import LanguageSelector from './components/LanguageSelector';
import { Tab, Category, CatDef, categories } from './nav';
import { clearToken, useApi } from './hooks/useApi';
import { useT } from './i18n';
import { ThemeProvider, ThemeSelector } from './theme';

/** Parse the URL hash (#category/tab) into a valid cat/tab pair, or null. */
function parseHash(): { cat: Category; tab: Tab } | null {
  const [catKey, tabKey] = window.location.hash.replace(/^#/, '').split('/');
  const cat = categories.find((c) => c.key === catKey);
  if (!cat) return null;
  const tab = cat.tabs.find((tb) => tb.key === tabKey);
  return { cat: cat.key, tab: tab ? tab.key : cat.tabs[0].key };
}

function App() {
  const { t } = useT();
  const [loggedIn, setLoggedIn] = useState(!!sessionStorage.getItem('nabiman_token'));
  const [activeTab, setActiveTab] = useState<Tab>('server');
  const [activeCat, setActiveCat] = useState<Category>('dashboard');
  const [sshTarget, setSshTarget] = useState<{ host: string; port: number; user: string } | null>(null);
  const [showChangePw, setShowChangePw] = useState(false);
  const [showPalette, setShowPalette] = useState(false);
  const [hiddenTabs, setHiddenTabs] = useState<Set<Tab>>(new Set());
  // Focus mode: hide the top bars + left sidebar so only the content frame shows.
  // Also drives the browser Fullscreen API for a true edge-to-edge view.
  const [focusMode, setFocusMode] = useState(false);

  const enterFocus = useCallback(() => {
    setFocusMode(true);
    const el = document.documentElement;
    if (el.requestFullscreen) el.requestFullscreen().catch(() => { /* denied/unsupported */ });
  }, []);
  const exitFocus = useCallback(() => {
    setFocusMode(false);
    if (document.fullscreenElement && document.exitFullscreen) {
      document.exitFullscreen().catch(() => {});
    }
  }, []);

  // Keep focus mode in sync when the browser leaves fullscreen (Esc / F11 / UI).
  useEffect(() => {
    const onFsChange = () => { if (!document.fullscreenElement) setFocusMode(false); };
    document.addEventListener('fullscreenchange', onFsChange);
    return () => document.removeEventListener('fullscreenchange', onFsChange);
  }, []);

  // Esc also exits when focus mode is on without browser fullscreen (denied).
  useEffect(() => {
    if (!focusMode) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') exitFocus(); };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [focusMode, exitFocus]);

  // Deep-linking: restore view from the URL hash on load and on back/forward.
  useEffect(() => {
    const apply = () => {
      const p = parseHash();
      if (p) { setActiveCat(p.cat); setActiveTab(p.tab); }
    };
    apply();
    window.addEventListener('hashchange', apply);
    return () => window.removeEventListener('hashchange', apply);
  }, []);
  // Keep the URL hash in sync so a refresh or shared link reopens the same view.
  useEffect(() => {
    const next = `#${activeCat}/${activeTab}`;
    if (window.location.hash !== next) window.history.replaceState(null, '', next);
  }, [activeCat, activeTab]);

  // Ctrl/Cmd+K opens the command palette from anywhere.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setShowPalette((v) => !v);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const { data: dockerAvailable } = useApi<boolean>(loggedIn ? '/api/containers/available' : '');
  const { data: dbAvailable } = useApi<boolean>(loggedIn ? '/api/database/available' : '');
  const { data: mailAvailable } = useApi<boolean>(loggedIn ? '/api/mail/available' : '');
  const { data: dnsAvailable } = useApi<boolean>(loggedIn ? '/api/dns/available' : '');
  useEffect(() => {
    const hidden = new Set<Tab>();
    if (dockerAvailable === false) hidden.add('containers');
    if (dbAvailable === false) hidden.add('database');
    if (mailAvailable === false) hidden.add('mail');
    if (dnsAvailable === false) hidden.add('dns');
    setHiddenTabs(hidden);
  }, [dockerAvailable, dbAvailable, mailAvailable, dnsAvailable]);

  const visibleCategories = useMemo(() => {
    return categories
      .map(cat => ({ ...cat, tabs: cat.tabs.filter(tab => !hiddenTabs.has(tab.key)) }))
      .filter(cat => cat.tabs.length > 0);
  }, [hiddenTabs]);

  const handleLogout = () => { clearToken(); setLoggedIn(false); };
  const handleConnectSSH = (host: string, port: number, user: string) => {
    setSshTarget({ host, port, user });
    setActiveCat('system');
    setActiveTab('terminal');
  };
  const handleNavigate = (tab: Tab) => { setActiveCat('mec'); setActiveTab(tab); };
  const navigateTo = (cat: Category, tab: Tab) => { setActiveCat(cat); setActiveTab(tab); };
  const handleCatClick = (cat: CatDef) => {
    setActiveCat(cat.key);
    if (!cat.tabs.some(tb => tb.key === activeTab)) setActiveTab(cat.tabs[0].key);
  };

  if (!loggedIn) return <LoginScreen onLogin={() => setLoggedIn(true)} />;

  const currentCat = visibleCategories.find(c => c.key === activeCat) || visibleCategories[0];

  return (
    <div className={`app ${focusMode ? 'focus-mode' : ''}`}>
      <header className="app-header">
        <h1 className="logo-link" onClick={() => { setActiveCat('dashboard'); setActiveTab('server'); }}>{t('app.title')}</h1>
        <span className="subtitle">{t('app.subtitle')}</span>
        <div className="header-actions">
          <ThemeSelector compact />
          <LanguageSelector />
          <button className="btn btn-secondary" title={t('app.commandPalette')} onClick={() => setShowPalette(true)}>⌘K</button>
          <button className="btn btn-secondary" title={t('app.focusMode')} onClick={enterFocus}>⛶</button>
          <button className="btn btn-secondary" onClick={() => setShowChangePw(true)}>{t('app.changePassword')}</button>
          <button className="btn btn-secondary logout-btn" onClick={handleLogout}>{t('app.logout')}</button>
        </div>
      </header>
      <nav className="cat-nav">
        {visibleCategories.map((cat) => (
          <button
            key={cat.key}
            className={`cat-btn ${activeCat === cat.key ? 'active' : ''}`}
            onClick={() => handleCatClick(cat)}
          >
            {t(cat.labelKey)}
          </button>
        ))}
      </nav>
      <div className="app-body">
        {currentCat.tabs.length > 1 && (
          <aside className="sidebar">
            {currentCat.tabs.map((tab) => (
              <button
                key={tab.key}
                className={`sidebar-btn ${activeTab === tab.key ? 'active' : ''}`}
                onClick={() => setActiveTab(tab.key)}
              >
                {t(tab.labelKey)}
              </button>
            ))}
          </aside>
        )}
        <main className={`main-content ${currentCat.tabs.length <= 1 ? 'full-width' : ''}`}>
          <PanelOutlet
            activeTab={activeTab}
            onConnectSSH={handleConnectSSH}
            onNavigate={handleNavigate}
            onEnterFocus={enterFocus}
          />
        </main>
        {/* Terminal: always mounted, positioned over main when active.
            Uses visibility+offscreen instead of display:none to keep xterm.js buffer intact. */}
        <div className={`terminal-persist ${activeTab === 'terminal' ? 'terminal-persist-visible' : 'terminal-persist-hidden'}`}>
          <TerminalPanel sshTarget={sshTarget} onSshConnected={() => setSshTarget(null)} isVisible={activeTab === 'terminal'} />
        </div>
      </div>
      {showChangePw && <ChangePasswordModal onClose={() => setShowChangePw(false)} />}
      {showPalette && (
        <CommandPalette
          onClose={() => setShowPalette(false)}
          onSelect={navigateTo}
          hiddenTabs={hiddenTabs}
        />
      )}
      {focusMode && (
        <button className="focus-exit-btn" title={t('app.exitFocus')} onClick={exitFocus}>
          ⛶ {t('app.exitFocus')}
        </button>
      )}
    </div>
  );
}

function AppWithProviders() {
  return (
    <ThemeProvider>
      <App />
    </ThemeProvider>
  );
}

export default AppWithProviders;
