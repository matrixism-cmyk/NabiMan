import React, { useState } from 'react';
import './App.css';
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
import { clearToken, apiPost } from './hooks/useApi';
import { useT, LANG_LABELS, Lang } from './i18n';

type Tab = 'server' | 'network' | 'accounts' | 'config' | 'traffic' | 'packages'
  | 'containers' | 'services' | 'firewall' | 'logs' | 'cron' | 'processes' | 'disks'
  | 'remote' | 'terminal';

const tabKeys: { key: Tab; labelKey: string }[] = [
  { key: 'server', labelKey: 'tab.serverStatus' },
  { key: 'remote', labelKey: 'tab.remoteServers' },
  { key: 'processes', labelKey: 'tab.processes' },
  { key: 'disks', labelKey: 'tab.disks' },
  { key: 'network', labelKey: 'tab.network' },
  { key: 'containers', labelKey: 'tab.containers' },
  { key: 'services', labelKey: 'tab.services' },
  { key: 'firewall', labelKey: 'tab.firewall' },
  { key: 'accounts', labelKey: 'tab.accounts' },
  { key: 'config', labelKey: 'tab.config' },
  { key: 'traffic', labelKey: 'tab.traffic' },
  { key: 'packages', labelKey: 'tab.packages' },
  { key: 'logs', labelKey: 'tab.logs' },
  { key: 'cron', labelKey: 'tab.cron' },
  { key: 'terminal', labelKey: 'tab.terminal' },
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
  const [sshTarget, setSshTarget] = useState<{ host: string; port: number; user: string } | null>(null);
  const [showChangePw, setShowChangePw] = useState(false);

  const handleLogout = () => {
    clearToken();
    setLoggedIn(false);
  };

  const handleConnectSSH = (host: string, port: number, user: string) => {
    setSshTarget({ host, port, user });
    setActiveTab('terminal');
  };

  if (!loggedIn) {
    return <LoginScreen onLogin={() => setLoggedIn(true)} />;
  }

  return (
    <div className="app">
      <header className="app-header">
        <h1>{t('app.title')}</h1>
        <span className="subtitle">{t('app.subtitle')}</span>
        <div className="header-actions">
          <LanguageSelector />
          <button className="btn btn-secondary" onClick={() => setShowChangePw(true)}>{t('app.changePassword')}</button>
          <button className="btn btn-secondary logout-btn" onClick={handleLogout}>{t('app.logout')}</button>
        </div>
      </header>
      <nav className="tab-nav">
        {tabKeys.map((tab) => (
          <button
            key={tab.key}
            className={`tab-btn ${activeTab === tab.key ? 'active' : ''}`}
            onClick={() => setActiveTab(tab.key)}
          >
            {t(tab.labelKey)}
          </button>
        ))}
      </nav>
      <main className="main-content">
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
        {activeTab === 'remote' && <RemoteServersPanel onConnectSSH={handleConnectSSH} />}
        {activeTab === 'terminal' && <TerminalPanel sshTarget={sshTarget} onSshConnected={() => setSshTarget(null)} />}
      </main>
      {showChangePw && <ChangePasswordModal onClose={() => setShowChangePw(false)} />}
    </div>
  );
}

export default App;
