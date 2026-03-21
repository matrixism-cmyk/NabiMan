import React, { useState } from 'react';
import './App.css';
import LoginScreen from './components/LoginScreen';
import ServerStatusPanel from './components/ServerStatusPanel';
import NetworkPanel from './components/NetworkPanel';
import AccountsPanel from './components/AccountsPanel';
import ConfigPanel from './components/ConfigPanel';
import TrafficPanel from './components/TrafficPanel';
import PackagesPanel from './components/PackagesPanel';
import { clearToken } from './hooks/useApi';

type Tab = 'server' | 'network' | 'accounts' | 'config' | 'traffic' | 'packages';

const tabs: { key: Tab; label: string }[] = [
  { key: 'server', label: 'Server Status' },
  { key: 'network', label: 'Network' },
  { key: 'accounts', label: 'Accounts' },
  { key: 'config', label: 'Config' },
  { key: 'traffic', label: 'Traffic' },
  { key: 'packages', label: 'Packages' },
];

function App() {
  const [loggedIn, setLoggedIn] = useState(!!sessionStorage.getItem('nabiman_token'));
  const [activeTab, setActiveTab] = useState<Tab>('server');

  const handleLogout = () => {
    clearToken();
    setLoggedIn(false);
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
        {activeTab === 'accounts' && <AccountsPanel />}
        {activeTab === 'config' && <ConfigPanel />}
        {activeTab === 'traffic' && <TrafficPanel />}
        {activeTab === 'packages' && <PackagesPanel />}
      </main>
    </div>
  );
}

export default App;
