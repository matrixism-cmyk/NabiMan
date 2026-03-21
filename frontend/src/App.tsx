import React, { useState } from 'react';
import './App.css';
import ServerStatusPanel from './components/ServerStatusPanel';
import NetworkPanel from './components/NetworkPanel';
import AccountsPanel from './components/AccountsPanel';
import ConfigPanel from './components/ConfigPanel';
import TrafficPanel from './components/TrafficPanel';

type Tab = 'server' | 'network' | 'accounts' | 'config' | 'traffic';

const tabs: { key: Tab; label: string }[] = [
  { key: 'server', label: 'Server Status' },
  { key: 'network', label: 'Network' },
  { key: 'accounts', label: 'Accounts' },
  { key: 'config', label: 'Config' },
  { key: 'traffic', label: 'Traffic' },
];

function App() {
  const [activeTab, setActiveTab] = useState<Tab>('server');

  return (
    <div className="app">
      <header className="app-header">
        <h1>NabiMan</h1>
        <span className="subtitle">Server Management Dashboard</span>
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
      </main>
    </div>
  );
}

export default App;
