import React, { useState, useEffect, useMemo, useCallback } from 'react';
import './App.css';
import './components.css';
import './design.css';
import { TerminalSettingsProvider } from './components/terminal/settings';
import { TerminalWindowsProvider } from './components/terminal/TerminalWindows';
import MfaPanel from './components/MfaPanel';
import LoginScreen from './components/LoginScreen';
import ScopeResourcesPanel from './components/ScopeResourcesPanel';
import { MemberUser } from './components/MembershipEditor';
import PanelOutlet from './components/PanelOutlet';
import ChangePasswordModal from './components/ChangePasswordModal';
import HeaderMenu from './components/HeaderMenu';
import IdleWarning from './components/IdleWarning';
import { useIdleLogout } from './shared/auth/useIdleLogout';
import { useChromeHidden } from './shared/ui/useChromeHidden';
import RemoteShell, { isStandaloneRemote } from './components/remote/RemoteShell';
import ConsolePopout, { isConsolePopout } from './components/remote/ConsolePopout';
import { DEFAULT_GRANTS } from './components/remote/member';
import CommandPalette from './components/CommandPalette';
import LanguageSelector from './shared/ui/LanguageSelector';
import { Tab, Category, CatDef, categories } from './nav';
import { apiPost, clearToken, useApi } from './shared/api/useApi';
import { useT } from './i18n';
import { ThemeProvider, ThemeSelector } from './shared/theme';

/** Parse the URL hash (#category/tab) into a valid cat/tab pair, or null. */
function parseHash(): { cat: Category; tab: Tab } | null {
  const [catKey, tabKey] = window.location.hash.replace(/^#/, '').split('/');
  // The terminal and the fleet summary now live inside the remote workspace.
  if ((catKey === 'system' && tabKey === 'terminal') || (catKey === 'dashboard' && tabKey === 'multiserver')) {
    return { cat: 'remote', tab: 'remote' };
  }
  if (catKey === 'security' && tabKey === 'usermgmt') return { cat: 'administration', tab: 'usermgmt' };
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
  const [showChangePw, setShowChangePw] = useState(false);
  const [showPalette, setShowPalette] = useState(false);
  const [hiddenTabs, setHiddenTabs] = useState<Set<Tab>>(new Set());
  // Focus mode: hide the top bars + left sidebar so only the content frame shows.
  // Also drives the browser Fullscreen API for a true edge-to-edge view.
  const [focusMode, setFocusMode] = useState(false);
  const [chromeHidden, setChromeHidden] = useChromeHidden();

  const enterFocus = useCallback(() => {
    setFocusMode(true);
    const el = document.documentElement;
    // Esc belongs to the terminal (vim, less): in fullscreen, hold Esc to leave.
    const lockEscape = () => (navigator as Navigator & { keyboard?: { lock?: (keys: string[]) => Promise<void> } })
      .keyboard?.lock?.(['Escape']).catch(() => undefined);
    if (el.requestFullscreen) el.requestFullscreen().then(lockEscape).catch(() => { /* denied/unsupported */ });
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
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !(e.target instanceof Element && e.target.closest('.xterm'))) exitFocus();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [focusMode, exitFocus]);

  // /remote is the remote workspace on its own: no menus, so no view in the hash.
  const [standalone] = useState(isStandaloneRemote);
  // Deep-linking: restore view from the URL hash on load and on back/forward.
  useEffect(() => {
    if (standalone) return undefined;
    const apply = () => {
      const p = parseHash();
      if (p) { setActiveCat(p.cat); setActiveTab(p.tab); }
    };
    apply();
    window.addEventListener('hashchange', apply);
    return () => window.removeEventListener('hashchange', apply);
  }, [standalone]);
  // Keep the URL hash in sync so a refresh or shared link reopens the same view.
  useEffect(() => {
    if (standalone || isStandaloneRemote()) return;
    const next = `#${activeCat}/${activeTab}`;
    if (window.location.hash !== next) window.history.replaceState(null, '', next);
  }, [activeCat, activeTab, standalone]);

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

  const { data: currentUser, refetch: refetchCurrentUser } = useApi<MemberUser>(loggedIn ? '/api/users/me' : '');
  useEffect(() => {
    const changed = () => { refetchCurrentUser(); };
    window.addEventListener('nabiman_mfa_required', changed);
    return () => window.removeEventListener('nabiman_mfa_required', changed);
  }, [refetchCurrentUser]);
  // A remote-only member gets the remote workspace and nothing else, at /remote.
  const member = currentUser?.role === 'remote';
  const scoped = !!currentUser && currentUser.role !== 'admin' && !member && (currentUser.scope_restricted || currentUser.memberships.length > 0);
  const full = loggedIn && currentUser && !scoped && !member;
  const { data: dockerAvailable } = useApi<boolean>(full ? '/api/containers/available' : '');
  const { data: dbAvailable } = useApi<boolean>(full ? '/api/database/available' : '');
  const { data: mailAvailable } = useApi<boolean>(full ? '/api/mail/available' : '');
  const { data: dnsAvailable } = useApi<boolean>(full ? '/api/dns/available' : '');
  useEffect(() => {
    if (member && !isStandaloneRemote()) window.history.replaceState(null, '', '/remote');
  }, [member]);
  useEffect(() => {
    const hidden = new Set<Tab>();
    if (dockerAvailable === false) hidden.add('containers');
    if (dbAvailable === false) hidden.add('database');
    if (mailAvailable === false) hidden.add('mail');
    if (dnsAvailable === false) hidden.add('dns');
    if (currentUser?.role === 'viewer' || currentUser?.role === 'operator') {
      hidden.add('backup');
      hidden.add('audit');
      hidden.add('files');
      hidden.add('config');
      hidden.add('vhost');
      hidden.add('remote');
      hidden.add('claudeChat');
      hidden.add('appSettings');
      hidden.add('claudeSettings');
    }
    if (currentUser?.role === 'operator') {
      for (const tab of ['cron', 'backup', 'packages', 'database', 'usermgmt', 'enterpriseauth', 'accounts'] as Tab[]) {
        hidden.add(tab);
      }
    }
    if (scoped) {
      const allowed = new Set<Tab>(['usermgmt', 'scopeSettings', 'loginSessions', 'mfa', 'remote', 'mecTenants']);
      hidden.clear();
      categories.forEach(c => c.tabs.forEach(tab => { if (!allowed.has(tab.key)) hidden.add(tab.key); }));
    }
    setHiddenTabs(hidden);
  }, [dockerAvailable, dbAvailable, mailAvailable, dnsAvailable, currentUser, scoped]);

  const visibleCategories = useMemo(() => {
    return categories
      .map(cat => ({ ...cat, labelKey: cat.key === 'administration' && currentUser?.role !== 'admin' ? 'cat.scopeAdministration' : cat.labelKey, tabs: cat.tabs.filter(tab => scoped
        ? ['usermgmt', 'scopeSettings', 'loginSessions', 'mfa', 'remote', 'mecTenants'].includes(tab.key) : !hiddenTabs.has(tab.key)) }))
      .filter(cat => cat.tabs.length > 0);
  }, [hiddenTabs, scoped, currentUser?.role]);

  useEffect(() => {
    if (!loggedIn || !currentUser) return;
    const cat = visibleCategories.find(c => c.key === activeCat);
    if (!cat || !cat.tabs.some(t => t.key === activeTab)) {
      const fallback = visibleCategories[0];
      if (fallback) { setActiveCat(fallback.key); setActiveTab(fallback.tabs[0].key); }
    }
  }, [loggedIn, currentUser, visibleCategories, activeCat, activeTab]);

  const handleLogout = async () => {
    const request = apiPost('/api/auth/logout', {});
    clearToken();
    setLoggedIn(false);
    setShowChangePw(false);
    setShowPalette(false);
    exitFocus();
    try { await request; } catch { /* local logout completes even if offline */ }
  };
  const idle = useIdleLogout(Boolean(loggedIn && currentUser), handleLogout);
  const handleNavigate = (tab: Tab) => { setActiveCat('mec'); setActiveTab(tab); };
  const navigateTo = (cat: Category, tab: Tab) => { setActiveCat(cat); setActiveTab(tab); };
  const handleCatClick = (cat: CatDef) => {
    setActiveCat(cat.key);
    if (!cat.tabs.some(tb => tb.key === activeTab)) setActiveTab(cat.tabs[0].key);
  };

  if (!loggedIn) return <LoginScreen onLogin={() => setLoggedIn(true)} />;

  if (!currentUser) return <div className="panel loading">권한을 확인하는 중…</div>;

  if (currentUser.mfa_blocked) return <main className="app">
    <p role="alert">{t('mfa.policyGate')}</p>
    <MfaPanel />
    <div className="btn-group">
      <button className="btn btn-secondary" onClick={() => refetchCurrentUser()}>{t('mfa.checkPolicy')}</button>
      <button className="btn btn-secondary" onClick={handleLogout}>{t('app.logout')}</button>
    </div>
  </main>;

  if (isConsolePopout()) {
    return currentUser.role === 'admin'
      ? <ConsolePopout />
      : <div className="panel loading">{t('mfa.policyGate')}</div>;
  }

  if (standalone || member) return (
    <TerminalSettingsProvider key={currentUser.username} enabled={currentUser.role === 'admin' || member}>
    <TerminalWindowsProvider key={currentUser.username} owner={currentUser.username} active={currentUser.role === 'admin' || member}>
      <RemoteShell username={currentUser.username} allowed={(currentUser.role === 'admin' && !scoped) || member}
        member={member} onChangePassword={member ? () => setShowChangePw(true) : undefined} onLogout={handleLogout}
        grants={member ? { ...DEFAULT_GRANTS, ...currentUser.remote_permissions } : null} />
      {showChangePw && <ChangePasswordModal onClose={() => setShowChangePw(false)} />}
      {idle.secondsLeft !== null && <IdleWarning seconds={idle.secondsLeft} onStay={idle.stay} />}
    </TerminalWindowsProvider>
    </TerminalSettingsProvider>
  );

  const currentCat = visibleCategories.find(c => c.key === activeCat) || visibleCategories[0];

  const visibleTab = currentCat.tabs.some(tab => tab.key === activeTab) ? activeTab : currentCat.tabs[0].key;

  return (
    <TerminalSettingsProvider key={currentUser.username} enabled={currentUser.role === 'admin'}>
    <TerminalWindowsProvider key={currentUser.username} owner={currentUser.username} active={currentUser.role === 'admin'}>
    <div className={`app ${focusMode ? 'focus-mode' : ''} ${chromeHidden ? 'chrome-hidden' : ''}`}>
      {chromeHidden && !focusMode && (
        <button type="button" className="chrome-handle" onClick={() => setChromeHidden(false)}
          title={t('app.showChrome')} aria-label={t('app.showChrome')}>▾</button>
      )}
      <header className="app-header">
        <h1><button className="logo-link" onClick={() => navigateTo(scoped ? 'administration' : 'dashboard', scoped ? 'usermgmt' : 'server')}>
          <img src={`${process.env.PUBLIC_URL}/favicon.svg`} alt="" width="32" height="32" />
          {t('app.title')}
        </button></h1>
        <span className="subtitle">{t('app.subtitle')}</span>
        <HeaderMenu>
          <ThemeSelector compact />
          <LanguageSelector />
          <button className="btn btn-secondary" title={t('app.commandPalette')} onClick={() => setShowPalette(true)}>⌘K</button>
          <button className="btn btn-secondary" title={t('app.focusMode')} onClick={enterFocus}>⛶</button>
          <button className="btn btn-secondary" onClick={() => setShowChangePw(true)}>{t('app.changePassword')}</button>
          <button className="btn btn-secondary logout-btn" onClick={handleLogout}>{t('app.logout')}</button>
        </HeaderMenu>
      </header>
      <nav className="cat-nav">
        {visibleCategories.map((cat) => (
          <button
            key={cat.key}
            className={`cat-btn ${activeCat === cat.key ? 'active' : ''}`}
            onClick={() => handleCatClick(cat)}
            aria-current={activeCat === cat.key ? 'page' : undefined}
          >
            {t(cat.labelKey)}
          </button>
        ))}
      </nav>
      <div className="app-body">
        {/* Folding the logo and the menu away: a small tab in the gap under the
            menu, where the one that unfolds them (▾, at the top) will be. */}
        {!chromeHidden && !focusMode && (
          <button type="button" className="chrome-fold" onClick={() => setChromeHidden(true)}
            title={t('app.hideChrome')} aria-label={t('app.hideChrome')}>▴</button>
        )}
        {currentCat.tabs.length > 1 && (
          <aside className="sidebar">
            <div className="sidebar-heading">{t(currentCat.labelKey)}</div>
            {currentCat.tabs.map((tab) => (
              <button
                key={tab.key}
                className={`sidebar-btn ${activeTab === tab.key ? 'active' : ''}`}
                onClick={() => setActiveTab(tab.key)}
                aria-current={activeTab === tab.key ? 'page' : undefined}
              >
                {t(tab.labelKey)}
              </button>
            ))}
          </aside>
        )}
        <main className={`main-content ${currentCat.tabs.length <= 1 ? 'full-width' : ''}`}>
          {scoped && (visibleTab === 'remote' || visibleTab === 'mecTenants')
            ? <ScopeResourcesPanel key={visibleTab} kind={visibleTab === 'remote' ? 'server' : 'tenant'} />
            : <PanelOutlet
            activeTab={visibleTab}
            onNavigate={handleNavigate}
            onEnterFocus={enterFocus}
          />}
        </main>
      </div>
      {showChangePw && <ChangePasswordModal onClose={() => setShowChangePw(false)} />}
      {idle.secondsLeft !== null && <IdleWarning seconds={idle.secondsLeft} onStay={idle.stay} />}
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
    </TerminalWindowsProvider>
    </TerminalSettingsProvider>
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
