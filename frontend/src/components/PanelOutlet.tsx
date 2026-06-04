import React from 'react';
import { Tab } from '../nav';
import ServerStatusPanel from './ServerStatusPanel';
import NetworkPanel from './NetworkPanel';
import AccountsPanel from './AccountsPanel';
import ConfigPanel from './ConfigPanel';
import TrafficPanel from './TrafficPanel';
import PackagesPanel from './PackagesPanel';
import ContainersPanel from './ContainersPanel';
import ServicesPanel from './ServicesPanel';
import FirewallPanel from './FirewallPanel';
import LogsPanel from './LogsPanel';
import CronPanel from './CronPanel';
import ProcessesPanel from './ProcessesPanel';
import DisksPanel from './DisksPanel';
import RemoteServersPanel from './RemoteServersPanel';
import UpdatesPanel from './UpdatesPanel';
import DiagnosticsPanel from './DiagnosticsPanel';
import SslPanel from './SslPanel';
import ChartsPanel from './ChartsPanel';
import FileManagerPanel from './FileManagerPanel';
import BackupPanel from './BackupPanel';
import AuditPanel from './AuditPanel';
import DatabasePanel from './DatabasePanel';
import MailPanel from './MailPanel';
import SwapPanel from './SwapPanel';
import SessionsPanel from './SessionsPanel';
import DnsPanel from './DnsPanel';
import UserManagementPanel from './UserManagementPanel';
import NotificationsPanel from './NotificationsPanel';
import AlertRulesPanel from './AlertRulesPanel';
import VhostPanel from './VhostPanel';
import IpBlockPanel from './IpBlockPanel';
import LicensePanel from './LicensePanel';
import EnterpriseAuthPanel from './EnterpriseAuthPanel';
import MultiServerDashboard from './MultiServerDashboard';
import MecDashboardPanel from './mec/MecDashboardPanel';
import MecNocPanel from './mec/MecNocPanel';
import MecLivePanel from './mec/MecLivePanel';
import TenantsPanel from './mec/TenantsPanel';
import MecNodesPanel from './mec/MecNodesPanel';
import MecGpuPanel from './mec/MecGpuPanel';
import MecFirewallPanel from './mec/MecFirewallPanel';
import MecAuditPanel from './mec/MecAuditPanel';
import MecJobsPanel from './mec/MecJobsPanel';
import MecStoragePanel from './mec/MecStoragePanel';
import MecHealthPanel from './mec/MecHealthPanel';
import MecSettingsPanel from './mec/MecSettingsPanel';
import MecIngressPanel from './mec/MecIngressPanel';
import MecDiscoveryPanel from './mec/MecDiscoveryPanel';
import MecUsersPanel from './mec/MecUsersPanel';

interface Props {
  activeTab: Tab;
  onConnectSSH: (host: string, port: number, user: string) => void;
  onNavigate: (tab: Tab) => void;
  onEnterFocus: () => void;
}

/// Renders the panel for the active tab. Extracted from App.tsx so the shell
/// stays within the line budget; the terminal panel stays in App because it is
/// always-mounted (persistent xterm buffer).
export default function PanelOutlet({ activeTab, onConnectSSH, onNavigate, onEnterFocus }: Props) {
  switch (activeTab) {
    case 'server': return <ServerStatusPanel />;
    case 'network': return <NetworkPanel />;
    case 'containers': return <ContainersPanel />;
    case 'services': return <ServicesPanel />;
    case 'firewall': return <FirewallPanel />;
    case 'accounts': return <AccountsPanel />;
    case 'config': return <ConfigPanel />;
    case 'traffic': return <TrafficPanel />;
    case 'packages': return <PackagesPanel />;
    case 'logs': return <LogsPanel />;
    case 'cron': return <CronPanel />;
    case 'processes': return <ProcessesPanel />;
    case 'disks': return <DisksPanel />;
    case 'charts': return <ChartsPanel />;
    case 'updates': return <UpdatesPanel />;
    case 'diagnostics': return <DiagnosticsPanel />;
    case 'ssl': return <SslPanel />;
    case 'files': return <FileManagerPanel />;
    case 'backup': return <BackupPanel />;
    case 'audit': return <AuditPanel />;
    case 'database': return <DatabasePanel />;
    case 'mail': return <MailPanel />;
    case 'swap': return <SwapPanel />;
    case 'sessions': return <SessionsPanel />;
    case 'dns': return <DnsPanel />;
    case 'usermgmt': return <UserManagementPanel />;
    case 'notifications': return <NotificationsPanel />;
    case 'alertrules': return <AlertRulesPanel />;
    case 'vhost': return <VhostPanel />;
    case 'ipblock': return <IpBlockPanel />;
    case 'license': return <LicensePanel />;
    case 'enterpriseauth': return <EnterpriseAuthPanel />;
    case 'multiserver': return <MultiServerDashboard />;
    case 'remote': return <RemoteServersPanel onConnectSSH={onConnectSSH} />;
    case 'mecNoc': return <MecNocPanel onNavigate={(tab) => onNavigate(tab as Tab)} onEnterFocus={onEnterFocus} />;
    case 'mecDashboard': return <MecDashboardPanel onNavigate={(tab) => onNavigate(tab as Tab)} />;
    case 'mecLive': return <MecLivePanel />;
    case 'mecTenants': return <TenantsPanel />;
    case 'mecNodes': return <MecNodesPanel />;
    case 'mecGpu': return <MecGpuPanel />;
    case 'mecFirewall': return <MecFirewallPanel />;
    case 'mecAudit': return <MecAuditPanel />;
    case 'mecJobs': return <MecJobsPanel />;
    case 'mecStorage': return <MecStoragePanel />;
    case 'mecHealth': return <MecHealthPanel />;
    case 'mecSettings': return <MecSettingsPanel />;
    case 'mecIngress': return <MecIngressPanel />;
    case 'mecDiscovery': return <MecDiscoveryPanel />;
    case 'mecUsers': return <MecUsersPanel />;
    default: return null;
  }
}
