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
import { clearToken } from './hooks/useApi';

type Tab = 'server' | 'network' | 'accounts' | 'config' | 'traffic' | 'packages'
  | 'containers' | 'services' | 'firewall' | 'logs' | 'cron' | 'processes' | 'disks'
  | 'remote' | 'terminal';

const tabs: { key: Tab; label: string }[] = [
  { key: 'server', label: 'Server Status' },
  { key: 'remote', label: 'Remote Servers' },
  { key: 'processes', label: 'Processes' },
  { key: 'disks', label: 'Disks' },
  { key: 'network', label: 'Network' },
  { key: 'containers', label: 'Containers' },
  { key: 'services', label: 'Services' },
  { key: 'firewall', label: 'Firewall' },
  { key: 'accounts', label: 'Accounts' },
  { key: 'config', label: 'Config' },
  { key: 'traffic', label: 'Traffic' },
  { key: 'packages', label: 'Packages' },
  { key: 'logs', label: 'Logs' },
  { key: 'cron', label: 'Cron' },
  { key: 'terminal', label: 'Terminal' },
];

function App() {
  const [loggedIn, setLoggedIn] = useState(!!sessionStorage.getItem('nabiman_token'));
  const [activeTab, setActiveTab] = useState<Tab>('server');
  const [sshTarget, setSshTarget] = useState<{ host: string; port: number; user: string } | null>(null);

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
        <h1>NabiMan</h1>
        <span className="subtitle">Server Management Dashboard</span>
        <button className="btn btn-secondary logout-btn" onClick={handleLogout}>Logout</button>
      </header>
      <nav className="tab-nav">
        {tabs.map((tab) => (
          <button
            key={tab.key}
            className={`tab-btn ${activeTab === tab.key ? 'active' : ''}`}
            onClick={() => setActiveTab(tab.key)}
          >
            {tab.label}
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
    </div>
  );
}

export default App;
