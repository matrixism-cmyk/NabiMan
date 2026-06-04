// Navigation model shared by the App shell and the PanelOutlet renderer.
// Extracted from App.tsx to keep the shell under the line budget.

export type Tab =
  | 'server' | 'charts' | 'network' | 'accounts' | 'config' | 'traffic' | 'packages'
  | 'containers' | 'services' | 'firewall' | 'logs' | 'cron' | 'processes' | 'disks'
  | 'remote' | 'terminal' | 'updates' | 'diagnostics' | 'ssl'
  | 'files' | 'backup' | 'audit' | 'database' | 'mail' | 'swap' | 'sessions' | 'dns' | 'usermgmt'
  | 'notifications' | 'alertrules' | 'vhost' | 'ipblock' | 'license' | 'enterpriseauth' | 'multiserver'
  | 'mecDashboard' | 'mecNoc' | 'mecLive' | 'mecTenants' | 'mecNodes' | 'mecGpu' | 'mecFirewall'
  | 'mecStorage' | 'mecIngress' | 'mecJobs' | 'mecAudit' | 'mecHealth'
  | 'mecSettings' | 'mecDiscovery' | 'mecUsers';

export type Category =
  | 'dashboard' | 'monitoring' | 'management' | 'security' | 'system' | 'remote' | 'mec';

export interface CatDef {
  key: Category;
  labelKey: string;
  tabs: { key: Tab; labelKey: string }[];
}

export const categories: CatDef[] = [
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
];
