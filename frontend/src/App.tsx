import React, { useState, useEffect, useMemo } from 'react';
import './App.css';
import './components.css';
import LoginScreen from './components/LoginScreen';
import ServerStatusPanel from './components/ServerStatusPanel';
import NetworkPanel from './components/NetworkPanel';
import AccountsPanel from './components/AccountsPanel';
import ConfigPanel from './components/ConfigPanel';
import TrafficPanel from './components/TrafficPanel';
import PackagesPanel from './components/PackagesPanel';
import ContainersPanel from './components/ContainersPanel';
import ServicesPanel from './components/ServicesPanel';
import FirewallPanel from './components/FirewallPanel';
import LogsPanel from './components/LogsPanel';
import CronPanel from './components/CronPanel';
import ProcessesPanel from './components/ProcessesPanel';
import DisksPanel from './components/DisksPanel';
import RemoteServersPanel from './components/RemoteServersPanel';
import TerminalPanel from './components/TerminalPanel';
import UpdatesPanel from './components/UpdatesPanel';
import DiagnosticsPanel from './components/DiagnosticsPanel';
import SslPanel from './components/SslPanel';
import ChartsPanel from './components/ChartsPanel';
import FileManagerPanel from './components/FileManagerPanel';
import BackupPanel from './components/BackupPanel';
import AuditPanel from './components/AuditPanel';
import DatabasePanel from './components/DatabasePanel';
import MailPanel from './components/MailPanel';
import SwapPanel from './components/SwapPanel';
import SessionsPanel from './components/SessionsPanel';
import DnsPanel from './components/DnsPanel';
import UserManagementPanel from './components/UserManagementPanel';
import NotificationsPanel from './components/NotificationsPanel';
import AlertRulesPanel from './components/AlertRulesPanel';
import VhostPanel from './components/VhostPanel';
import IpBlockPanel from './components/IpBlockPanel';
import LicensePanel from './components/LicensePanel';
import EnterpriseAuthPanel from './components/EnterpriseAuthPanel';
import MultiServerDashboard from './components/MultiServerDashboard';
import MecDashboardPanel from './components/mec/MecDashboardPanel';
import TenantsPanel from './components/mec/TenantsPanel';
import MecNodesPanel from './components/mec/MecNodesPanel';
import MecGpuPanel from './components/mec/MecGpuPanel';
import MecFirewallPanel from './components/mec/MecFirewallPanel';
import MecAuditPanel from './components/mec/MecAuditPanel';
import MecJobsPanel from './components/mec/MecJobsPanel';
import MecStoragePanel from './components/mec/MecStoragePanel';
import MecHealthPanel from './components/mec/MecHealthPanel';
import MecSettingsPanel from './components/mec/MecSettingsPanel';
import MecIngressPanel from './components/mec/MecIngressPanel';
import MecDiscoveryPanel from './components/mec/MecDiscoveryPanel';
import MecUsersPanel from './components/mec/MecUsersPanel';
import { clearToken, apiPost, useApi } from './hooks/useApi';
import { useT, LANG_LABELS, Lang } from './i18n';
import { ThemeProvider, ThemeSelector } from './theme';

type Tab = 'server' | 'charts' | 'network' | 'accounts' | 'config' | 'traffic' | 'packages'
  | 'containers' | 'services' | 'firewall' | 'logs' | 'cron' | 'processes' | 'disks'
  | 'remote' | 'terminal' | 'updates' | 'diagnostics' | 'ssl'
  | 'files' | 'backup' | 'audit' | 'database' | 'mail' | 'swap' | 'sessions' | 'dns' | 'usermgmt'
  | 'notifications' | 'alertrules' | 'vhost' | 'ipblock' | 'license' | 'enterpriseauth' | 'multiserver'
  | 'mecDashboard' | 'mecTenants' | 'mecNodes' | 'mecGpu' | 'mecFirewall'
  | 'mecStorage' | 'mecIngress' | 'mecJobs' | 'mecAudit' | 'mecHealth'
  | 'mecSettings' | 'mecDiscovery' | 'mecUsers';

type Category = 'dashboard' | 'monitoring' | 'management' | 'security' | 'system' | 'remote' | 'mec';

interface CatDef {
  key: Category;
  labelKey: string;
  tabs: { key: Tab; labelKey: string }[];
}

const categories: CatDef[] = [
  { key: 'dashboard', labelKey: 'cat.dashboard', tabs: [
    { key: 'server', labelKey: 'tab.serverStatus' },
    { key: 'charts', labelKey: 'tab.charts' },
    { key: 'multiserver', labelKey: 'tab.multiServer' },
  ]},
  { key: 'monitoring', labelKey: 'cat.monitoring', tabs: [
    { key: 'processes', labelKey: 'tab.processes' },
    { key: 'disks', labelKey: 'tab.disks' },
    { key: 'swap', labelKey: 'tab.swap' },
    { key: 'network', labelKey: 'tab.network' },
    { key: 'traffic', labelKey: 'tab.traffic' },
    { key: 'dns', labelKey: 'tab.dns' },
    { key: 'mail', labelKey: 'tab.mail' },
    { key: 'diagnostics', labelKey: 'tab.diagnostics' },
  ]},
  { key: 'management', labelKey: 'cat.management', tabs: [
    { key: 'containers', labelKey: 'tab.containers' },
    { key: 'services', labelKey: 'tab.services' },
    { key: 'packages', labelKey: 'tab.packages' },
    { key: 'updates', labelKey: 'tab.updates' },
    { key: 'database', labelKey: 'tab.database' },
    { key: 'vhost', labelKey: 'tab.vhost' },
    { key: 'config', labelKey: 'tab.config' },
    { key: 'backup', labelKey: 'tab.backup' },
    { key: 'notifications', labelKey: 'tab.notifications' },
    { key: 'alertrules', labelKey: 'tab.alertRules' },
  ]},
  { key: 'security', labelKey: 'cat.security', tabs: [
    { key: 'firewall', labelKey: 'tab.firewall' },
    { key: 'ipblock', labelKey: 'tab.ipBlock' },
    { key: 'accounts', labelKey: 'tab.accounts' },
    { key: 'ssl', labelKey: 'tab.ssl' },
    { key: 'usermgmt', labelKey: 'tab.userMgmt' },
    { key: 'sessions', labelKey: 'tab.sessions' },
    { key: 'audit', labelKey: 'tab.audit' },
    { key: 'enterpriseauth', labelKey: 'tab.enterpriseAuth' },
  ]},
  { key: 'system', labelKey: 'cat.system', tabs: [
    { key: 'logs', labelKey: 'tab.logs' },
    { key: 'cron', labelKey: 'tab.cron' },
    { key: 'files', labelKey: 'tab.files' },
    { key: 'terminal', labelKey: 'tab.terminal' },
    { key: 'license', labelKey: 'tab.license' },
  ]},
  { key: 'remote', labelKey: 'cat.remote', tabs: [
    { key: 'remote', labelKey: 'tab.remoteServers' },
  ]},
  { key: 'mec', labelKey: 'cat.mec', tabs: [
    { key: 'mecDashboard', labelKey: 'tab.mecDashboard' },
    { key: 'mecHealth', labelKey: 'tab.mecHealth' },
    { key: 'mecDiscovery', labelKey: 'tab.mecDiscovery' },
    { key: 'mecTenants', labelKey: 'tab.mecTenants' },
    { key: 'mecUsers', labelKey: 'tab.mecUsers' },
    { key: 'mecNodes', labelKey: 'tab.mecNodes' },
    { key: 'mecGpu', labelKey: 'tab.mecGpu' },
    { key: 'mecFirewall', labelKey: 'tab.mecFirewall' },
    { key: 'mecIngress', labelKey: 'tab.mecIngress' },
    { key: 'mecStorage', labelKey: 'tab.mecStorage' },
    { key: 'mecJobs', labelKey: 'tab.mecJobs' },
    { key: 'mecAudit', labelKey: 'tab.mecAudit' },
    { key: 'mecSettings', labelKey: 'tab.mecSettings' },
  ]},
];

function ChangePasswordModal({ onClose }: { onClose: () => void }) {
  const { t } = useT();
  const [currentPw, setCurrentPw] = useState('');
  const [newPw, setNewPw] = useState('');
  const [confirmPw, setConfirmPw] = useState('');
  const [msg, setMsg] = useState('');
  const [success, setSuccess] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setMsg('');
    if (newPw !== confirmPw) { setMsg(t('app.passwordMismatch')); return; }
    if (newPw.length < 4) { setMsg(t('app.passwordTooShort')); return; }
    const res = await apiPost<string>('/api/auth/change-password', { current_password: currentPw, new_password: newPw });
    if (res.success) { setSuccess(true); setMsg(t('app.passwordChanged')); }
    else { setMsg(res.message || t('app.failed')); }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content" onClick={e => e.stopPropagation()}>
        <h3>{t('app.changePassword')}</h3>
        <form onSubmit={handleSubmit}>
          <input type="password" placeholder={t('app.currentPassword')} value={currentPw} onChange={e => setCurrentPw(e.target.value)} required autoFocus />
          <input type="password" placeholder={t('app.newPassword')} value={newPw} onChange={e => setNewPw(e.target.value)} required />
          <input type="password" placeholder={t('app.confirmPassword')} value={confirmPw} onChange={e => setConfirmPw(e.target.value)} required />
          {msg && <div className={success ? 'login-success' : 'login-error'}>{msg}</div>}
          <div className="modal-actions">
            {!success && <button type="submit" className="btn btn-primary">{t('common.change')}</button>}
            <button type="button" className="btn btn-secondary" onClick={onClose}>{success ? t('common.close') : t('common.cancel')}</button>
          </div>
        </form>
      </div>
    </div>
  );
}

function LanguageSelector() {
  const { lang, setLang, t } = useT();
  const langs = Object.entries(LANG_LABELS) as [Lang, string][];

  return (
    <div className="lang-selector">
      <select
        value={lang}
        onChange={e => setLang(e.target.value as Lang)}
        className="lang-select"
        title={t('common.language')}
      >
        {langs.map(([code, label]) => (
          <option key={code} value={code}>{label}</option>
        ))}
      </select>
    </div>
  );
}

function App() {
  const { t } = useT();
  const [loggedIn, setLoggedIn] = useState(!!sessionStorage.getItem('nabiman_token'));
  const [activeTab, setActiveTab] = useState<Tab>('server');
  const [activeCat, setActiveCat] = useState<Category>('dashboard');
  const [sshTarget, setSshTarget] = useState<{ host: string; port: number; user: string } | null>(null);
  const [showChangePw, setShowChangePw] = useState(false);
  const [hiddenTabs, setHiddenTabs] = useState<Set<Tab>>(new Set());

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

  const handleLogout = () => {
    clearToken();
    setLoggedIn(false);
  };

  const handleConnectSSH = (host: string, port: number, user: string) => {
    setSshTarget({ host, port, user });
    setActiveCat('system');
    setActiveTab('terminal');
  };

  const handleCatClick = (cat: CatDef) => {
    setActiveCat(cat.key);
    if (!cat.tabs.some(t => t.key === activeTab)) {
      setActiveTab(cat.tabs[0].key);
    }
  };

  if (!loggedIn) {
    return <LoginScreen onLogin={() => setLoggedIn(true)} />;
  }

  const currentCat = visibleCategories.find(c => c.key === activeCat) || visibleCategories[0];

  return (
    <div className="app">
      <header className="app-header">
        <h1 className="logo-link" onClick={() => { setActiveCat('dashboard'); setActiveTab('server'); }}>{t('app.title')}</h1>
        <span className="subtitle">{t('app.subtitle')}</span>
        <div className="header-actions">
          <ThemeSelector compact />
          <LanguageSelector />
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
          {activeTab === 'server' && <ServerStatusPanel />}
          {activeTab === 'network' && <NetworkPanel />}
          {activeTab === 'containers' && <ContainersPanel />}
          {activeTab === 'services' && <ServicesPanel />}
          {activeTab === 'firewall' && <FirewallPanel />}
          {activeTab === 'accounts' && <AccountsPanel />}
          {activeTab === 'config' && <ConfigPanel />}
          {activeTab === 'traffic' && <TrafficPanel />}
          {activeTab === 'packages' && <PackagesPanel />}
          {activeTab === 'logs' && <LogsPanel />}
          {activeTab === 'cron' && <CronPanel />}
          {activeTab === 'processes' && <ProcessesPanel />}
          {activeTab === 'disks' && <DisksPanel />}
          {activeTab === 'charts' && <ChartsPanel />}
          {activeTab === 'updates' && <UpdatesPanel />}
          {activeTab === 'diagnostics' && <DiagnosticsPanel />}
          {activeTab === 'ssl' && <SslPanel />}
          {activeTab === 'files' && <FileManagerPanel />}
          {activeTab === 'backup' && <BackupPanel />}
          {activeTab === 'audit' && <AuditPanel />}
          {activeTab === 'database' && <DatabasePanel />}
          {activeTab === 'mail' && <MailPanel />}
          {activeTab === 'swap' && <SwapPanel />}
          {activeTab === 'sessions' && <SessionsPanel />}
          {activeTab === 'dns' && <DnsPanel />}
          {activeTab === 'usermgmt' && <UserManagementPanel />}
          {activeTab === 'notifications' && <NotificationsPanel />}
          {activeTab === 'alertrules' && <AlertRulesPanel />}
          {activeTab === 'vhost' && <VhostPanel />}
          {activeTab === 'ipblock' && <IpBlockPanel />}
          {activeTab === 'license' && <LicensePanel />}
          {activeTab === 'enterpriseauth' && <EnterpriseAuthPanel />}
          {activeTab === 'multiserver' && <MultiServerDashboard />}
          {activeTab === 'remote' && <RemoteServersPanel onConnectSSH={handleConnectSSH} />}
          {activeTab === 'mecDashboard' && <MecDashboardPanel />}
          {activeTab === 'mecTenants' && <TenantsPanel />}
          {activeTab === 'mecNodes' && <MecNodesPanel />}
          {activeTab === 'mecGpu' && <MecGpuPanel />}
          {activeTab === 'mecFirewall' && <MecFirewallPanel />}
          {activeTab === 'mecAudit' && <MecAuditPanel />}
          {activeTab === 'mecJobs' && <MecJobsPanel />}
          {activeTab === 'mecStorage' && <MecStoragePanel />}
          {activeTab === 'mecHealth' && <MecHealthPanel />}
          {activeTab === 'mecSettings' && <MecSettingsPanel />}
          {activeTab === 'mecIngress' && <MecIngressPanel />}
          {activeTab === 'mecDiscovery' && <MecDiscoveryPanel />}
          {activeTab === 'mecUsers' && <MecUsersPanel />}
        </main>
        {/* Terminal: always mounted, positioned over main when active.
            Uses visibility+offscreen instead of display:none to keep xterm.js buffer intact. */}
        <div className={`terminal-persist ${activeTab === 'terminal' ? 'terminal-persist-visible' : 'terminal-persist-hidden'}`}>
          <TerminalPanel sshTarget={sshTarget} onSshConnected={() => setSshTarget(null)} isVisible={activeTab === 'terminal'} />
        </div>
      </div>
      {showChangePw && <ChangePasswordModal onClose={() => setShowChangePw(false)} />}
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
