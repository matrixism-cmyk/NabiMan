// Navigation model shared by the App shell and the PanelOutlet renderer.
// Extracted from App.tsx to keep the shell under the line budget.

export type Tab =
  | 'server' | 'charts' | 'network' | 'accounts' | 'config' | 'traffic' | 'packages'
  | 'containers' | 'services' | 'firewall' | 'logs' | 'cron' | 'processes' | 'disks'
  | 'remote' | 'updates' | 'diagnostics' | 'ssl'
  | 'files' | 'backup' | 'audit' | 'database' | 'mail' | 'swap' | 'sessions' | 'dns' | 'usermgmt'
  | 'mfa' | 'loginSessions' | 'scopeSettings' | 'notifications' | 'alertrules' | 'vhost' | 'ipblock' | 'license' | 'enterpriseauth'
  | 'mecDashboard' | 'mecNoc' | 'mecLive' | 'mecTenants' | 'mecNodes' | 'mecGpu' | 'mecFirewall'
  | 'mecStorage' | 'mecIngress' | 'mecJobs' | 'mecAudit' | 'mecHealth'
  | 'mecSettings' | 'mecDiscovery' | 'mecUsers'
  | 'claudeChat' | 'claudeSettings' | 'appSettings';

export type Category =
  | 'administration' | 'dashboard' | 'monitoring' | 'management' | 'security' | 'system' | 'remote' | 'mec' | 'claude';

export interface CatDef {
  key: Category;
  labelKey: string;
  tabs: { key: Tab; labelKey: string }[];
}

export const categories: CatDef[] = [
  { key: 'administration', labelKey: 'cat.administration', tabs: [
    { key: 'usermgmt', labelKey: 'tab.userMgmt' },
    { key: 'scopeSettings', labelKey: 'tab.scopeSettings' },
    { key: 'loginSessions', labelKey: 'loginSessions.title' },
    { key: 'mfa', labelKey: 'mfa.title' },
    { key: 'appSettings', labelKey: 'tab.appSettings' },
  ]},
  { key: 'dashboard', labelKey: 'cat.dashboard', tabs: [
    { key: 'server', labelKey: 'tab.serverStatus' },
    { key: 'charts', labelKey: 'tab.charts' },
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
    { key: 'sessions', labelKey: 'tab.sessions' },
    { key: 'audit', labelKey: 'tab.audit' },
    { key: 'enterpriseauth', labelKey: 'tab.enterpriseAuth' },
  ]},
  { key: 'system', labelKey: 'cat.system', tabs: [
    { key: 'logs', labelKey: 'tab.logs' },
    { key: 'cron', labelKey: 'tab.cron' },
    { key: 'files', labelKey: 'tab.files' },
    { key: 'license', labelKey: 'tab.license' },
  ]},
  // One workspace: the server list, the terminal and the fleet summary used to
  // live in three different categories.
  { key: 'remote', labelKey: 'cat.remote', tabs: [
    { key: 'remote', labelKey: 'tab.remoteWorkspace' },
  ]},
  { key: 'mec', labelKey: 'cat.mec', tabs: [
    { key: 'mecNoc', labelKey: 'tab.mecNoc' },
    { key: 'mecDashboard', labelKey: 'tab.mecDashboard' },
    { key: 'mecLive', labelKey: 'tab.mecLive' },
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
  // Talking with Claude by voice; last, at the right end.
  { key: 'claude', labelKey: 'cat.claude', tabs: [
    { key: 'claudeChat', labelKey: 'tab.claudeChat' },
    { key: 'claudeSettings', labelKey: 'tab.claudeSettings' },
  ]},
];
