import React, { lazy, Suspense } from 'react';
import { Tab } from '../nav';
import { useT } from '../i18n';
import { ErrorBoundary } from '../shared/ui/ErrorBoundary';
// Eager: the first screen after login, and what App.tsx imports anyway.
import MfaPanel from './MfaPanel';
import ServerStatusPanel from './ServerStatusPanel';
import RemoteWorkspace from './remote/RemoteWorkspace';

// Every other panel is its own chunk, fetched the first time its tab opens.
const ClaudeChatPanel = lazy(() => import('./claude/ClaudeChatPanel'));
const AppSettingsPanel = lazy(() => import('./AppSettingsPanel'));
const ClaudeSettingsPanel = lazy(() => import('./claude/ClaudeSettingsPanel'));
const LoginSessionsPanel = lazy(() => import('./LoginSessionsPanel'));
const NetworkPanel = lazy(() => import('./NetworkPanel'));
const AccountsPanel = lazy(() => import('./AccountsPanel'));
const ConfigPanel = lazy(() => import('./ConfigPanel'));
const TrafficPanel = lazy(() => import('./TrafficPanel'));
const PackagesPanel = lazy(() => import('./PackagesPanel'));
const ContainersPanel = lazy(() => import('./ContainersPanel'));
const ServicesPanel = lazy(() => import('./ServicesPanel'));
const FirewallPanel = lazy(() => import('./FirewallPanel'));
const LogsPanel = lazy(() => import('./LogsPanel'));
const CronPanel = lazy(() => import('./CronPanel'));
const ProcessesPanel = lazy(() => import('./ProcessesPanel'));
const DisksPanel = lazy(() => import('./DisksPanel'));
const UpdatesPanel = lazy(() => import('./UpdatesPanel'));
const DiagnosticsPanel = lazy(() => import('./DiagnosticsPanel'));
const SslPanel = lazy(() => import('./SslPanel'));
const ChartsPanel = lazy(() => import('./ChartsPanel'));
const FileManagerPanel = lazy(() => import('./FileManagerPanel'));
const BackupPanel = lazy(() => import('./BackupPanel'));
const AuditPanel = lazy(() => import('./AuditPanel'));
const DatabasePanel = lazy(() => import('./DatabasePanel'));
const MailPanel = lazy(() => import('./MailPanel'));
const SwapPanel = lazy(() => import('./SwapPanel'));
const SessionsPanel = lazy(() => import('./SessionsPanel'));
const DnsPanel = lazy(() => import('./DnsPanel'));
const ScopeSettingsPanel = lazy(() => import('./ScopeSettingsPanel'));
const UserManagementPanel = lazy(() => import('./UserManagementPanel'));
const NotificationsPanel = lazy(() => import('./NotificationsPanel'));
const AlertRulesPanel = lazy(() => import('./AlertRulesPanel'));
const VhostPanel = lazy(() => import('./VhostPanel'));
const IpBlockPanel = lazy(() => import('./IpBlockPanel'));
const LicensePanel = lazy(() => import('./LicensePanel'));
const EnterpriseAuthPanel = lazy(() => import('./EnterpriseAuthPanel'));
const MecDashboardPanel = lazy(() => import('./mec/MecDashboardPanel'));
const MecNocPanel = lazy(() => import('./mec/MecNocPanel'));
const MecLivePanel = lazy(() => import('./mec/MecLivePanel'));
const TenantsPanel = lazy(() => import('./mec/TenantsPanel'));
const MecNodesPanel = lazy(() => import('./mec/MecNodesPanel'));
const MecGpuPanel = lazy(() => import('./mec/MecGpuPanel'));
const MecFirewallPanel = lazy(() => import('./mec/MecFirewallPanel'));
const MecAuditPanel = lazy(() => import('./mec/MecAuditPanel'));
const MecJobsPanel = lazy(() => import('./mec/MecJobsPanel'));
const MecStoragePanel = lazy(() => import('./mec/MecStoragePanel'));
const MecHealthPanel = lazy(() => import('./mec/MecHealthPanel'));
const MecSettingsPanel = lazy(() => import('./mec/MecSettingsPanel'));
const MecIngressPanel = lazy(() => import('./mec/MecIngressPanel'));
const MecDiscoveryPanel = lazy(() => import('./mec/MecDiscoveryPanel'));
const MecUsersPanel = lazy(() => import('./mec/MecUsersPanel'));

interface Props {
  activeTab: Tab;
  onNavigate: (tab: Tab) => void;
  onEnterFocus: () => void;
}

function PanelLoading() {
  const { t } = useT();
  return <div className="panel loading" role="status">{t('panel.loading')}</div>;
}

/// Renders the panel for the active tab, each in its own error boundary, with
/// a short "loading" while a panel's chunk arrives. Extracted from App.tsx so
/// the shell stays within the line budget.
export default function PanelOutlet({ activeTab, onNavigate, onEnterFocus }: Props) {
  return (
    <ErrorBoundary key={activeTab} region={`panel:${activeTab}`}>
      <Suspense fallback={<PanelLoading />}>
        {panelFor(activeTab, onNavigate, onEnterFocus)}
      </Suspense>
    </ErrorBoundary>
  );
}

function panelFor(activeTab: Tab, onNavigate: Props['onNavigate'], onEnterFocus: Props['onEnterFocus']) {
  switch (activeTab) {
    case 'mfa': return <MfaPanel />;
    case 'claudeChat': return <ClaudeChatPanel />;
    case 'appSettings': return <AppSettingsPanel />;
    case 'claudeSettings': return <ClaudeSettingsPanel />;
    case 'loginSessions': return <LoginSessionsPanel />;
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
    case 'scopeSettings': return <ScopeSettingsPanel />;
    case 'usermgmt': return <UserManagementPanel />;
    case 'notifications': return <NotificationsPanel />;
    case 'alertrules': return <AlertRulesPanel />;
    case 'vhost': return <VhostPanel />;
    case 'ipblock': return <IpBlockPanel />;
    case 'license': return <LicensePanel />;
    case 'enterpriseauth': return <EnterpriseAuthPanel />;
    case 'remote': return <RemoteWorkspace />;
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
