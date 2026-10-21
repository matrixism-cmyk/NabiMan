import React, { useState, useEffect } from 'react';
import { useApi, apiPost } from '../shared/api/useApi';
import { useT } from '../i18n';
import ApiTokensPanel from './ApiTokensPanel';

interface LdapConfig {
  enabled: boolean;
  server_url: string;
  bind_dn: string;
  bind_password: string;
  search_base: string;
  user_filter: string;
  tls: boolean;
  admin_group: string;
  operator_group: string;
}

interface OAuthConfig {
  enabled: boolean;
  provider_name: string;
  client_id: string;
  client_secret: string;
  authorize_url: string;
  token_url: string;
  userinfo_url: string;
  scopes: string;
  redirect_uri: string;
  role_claim: string;
  admin_value: string;
  operator_value: string;
}

type SubTab = 'ldap' | 'oauth' | 'apikeys';

/* ─── LDAP Sub-tab ─── */
function LdapTab() {
  const { t } = useT();
  const { data, loading, refetch } = useApi<LdapConfig>('/api/auth/ldap/config');
  const [cfg, setCfg] = useState<LdapConfig>({
    enabled: false, server_url: '', bind_dn: '', bind_password: '',
    search_base: '', user_filter: '', tls: false, admin_group: '', operator_group: '',
  });
  const [msg, setMsg] = useState('');
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);

  useEffect(() => { if (data) setCfg(data); }, [data]);

  const handleSave = async () => {
    setSaving(true); setMsg('');
    const res = await apiPost<string>('/api/auth/ldap/config', cfg);
    setMsg(res.data || res.message);
    setSaving(false); refetch();
  };

  const handleTest = async () => {
    setTesting(true); setMsg('');
    const res = await apiPost<string>('/api/auth/ldap/test', {});
    setMsg(res.data || res.message);
    setTesting(false);
  };

  const update = (key: keyof LdapConfig, value: string | boolean) =>
    setCfg(prev => ({ ...prev, [key]: value }));

  if (loading) return <div className="text-secondary">{t('common.loading')}</div>;

  return (
    <div>
      {msg && <div className="message" onClick={() => setMsg('')}>{msg}</div>}
      <div className="inline-form">
        <div className="form-row">
          <label>{t('ldap.enabled')}</label>
          <label style={{ display: 'flex', alignItems: 'center', gap: 6, minWidth: 'auto' }}>
            <input type="checkbox" checked={cfg.enabled} onChange={e => update('enabled', e.target.checked)} />
            {cfg.enabled ? t('ldap.on') : t('ldap.off')}
          </label>
        </div>
        <div className="form-row">
          <label>{t('ldap.serverUrl')}</label>
          <input value={cfg.server_url} onChange={e => update('server_url', e.target.value)}
            placeholder="ldap://ldap.example.com:389" />
        </div>
        <div className="form-row">
          <label>{t('ldap.bindDn')}</label>
          <input value={cfg.bind_dn} onChange={e => update('bind_dn', e.target.value)}
            placeholder="cn=admin,dc=example,dc=com" />
        </div>
        <div className="form-row">
          <label>{t('ldap.bindPassword')}</label>
          <input type="password" value={cfg.bind_password}
            onChange={e => update('bind_password', e.target.value)} />
        </div>
        <div className="form-row">
          <label>{t('ldap.searchBase')}</label>
          <input value={cfg.search_base} onChange={e => update('search_base', e.target.value)}
            placeholder="dc=example,dc=com" />
        </div>
        <div className="form-row">
          <label>{t('ldap.userFilter')}</label>
          <input value={cfg.user_filter} onChange={e => update('user_filter', e.target.value)}
            placeholder="(uid={username})" />
        </div>
        <div className="form-row">
          <label>TLS</label>
          <label style={{ display: 'flex', alignItems: 'center', gap: 6, minWidth: 'auto' }}>
            <input type="checkbox" checked={cfg.tls} onChange={e => update('tls', e.target.checked)} />
            {cfg.tls ? t('ldap.tlsOn') : t('ldap.tlsOff')}
          </label>
        </div>
        <div className="form-row">
          <label>{t('ldap.adminGroup')}</label>
          <input value={cfg.admin_group} onChange={e => update('admin_group', e.target.value)}
            placeholder="cn=admins,ou=groups,dc=example,dc=com" />
        </div>
        <div className="form-row">
          <label>{t('ldap.operatorGroup')}</label>
          <input value={cfg.operator_group} onChange={e => update('operator_group', e.target.value)}
            placeholder="cn=operators,ou=groups,dc=example,dc=com" />
        </div>
        <div className="btn-group">
          <button className="btn btn-primary" onClick={handleSave} disabled={saving}>
            {saving ? t('common.working') : t('common.save')}
          </button>
          <button className="btn btn-secondary" onClick={handleTest} disabled={testing}>
            {testing ? t('common.working') : t('ldap.testConnection')}
          </button>
        </div>
      </div>
    </div>
  );
}

/* ─── OAuth Sub-tab ─── */
function OAuthTab() {
  const { t } = useT();
  const { data, loading, refetch } = useApi<OAuthConfig>('/api/auth/oauth/config');
  const [cfg, setCfg] = useState<OAuthConfig>({
    enabled: false, provider_name: '', client_id: '', client_secret: '',
    authorize_url: '', token_url: '', userinfo_url: '', scopes: '',
    redirect_uri: '', role_claim: '', admin_value: '', operator_value: '',
  });
  const [msg, setMsg] = useState('');
  const [saving, setSaving] = useState(false);

  useEffect(() => { if (data) setCfg(data); }, [data]);

  const handleSave = async () => {
    setSaving(true); setMsg('');
    const res = await apiPost<string>('/api/auth/oauth/config', cfg);
    setMsg(res.data || res.message);
    setSaving(false); refetch();
  };

  const update = (key: keyof OAuthConfig, value: string | boolean) =>
    setCfg(prev => ({ ...prev, [key]: value }));

  if (loading) return <div className="text-secondary">{t('common.loading')}</div>;

  const fields: { key: keyof OAuthConfig; placeholder: string }[] = [
    { key: 'provider_name', placeholder: 'Google / GitHub / Okta' },
    { key: 'client_id', placeholder: 'client-id-xxx' },
    { key: 'client_secret', placeholder: 'client-secret-xxx' },
    { key: 'authorize_url', placeholder: 'https://provider.com/oauth/authorize' },
    { key: 'token_url', placeholder: 'https://provider.com/oauth/token' },
    { key: 'userinfo_url', placeholder: 'https://provider.com/oauth/userinfo' },
    { key: 'scopes', placeholder: 'openid profile email' },
    { key: 'redirect_uri', placeholder: 'https://your-server.com/' },
    { key: 'role_claim', placeholder: 'role' },
    { key: 'admin_value', placeholder: 'admin' },
    { key: 'operator_value', placeholder: 'operator' },
  ];

  return (
    <div>
      {msg && <div className="message" onClick={() => setMsg('')}>{msg}</div>}
      <div className="inline-form">
        <div className="form-row">
          <label>{t('oauth.enabled')}</label>
          <label style={{ display: 'flex', alignItems: 'center', gap: 6, minWidth: 'auto' }}>
            <input type="checkbox" checked={cfg.enabled} onChange={e => update('enabled', e.target.checked)} />
            {cfg.enabled ? t('oauth.on') : t('oauth.off')}
          </label>
        </div>
        {fields.map(({ key, placeholder }) => (
          <div className="form-row" key={key}>
            <label>{t(`oauth.${key}`)}</label>
            <input
              type={key === 'client_secret' ? 'password' : 'text'}
              value={cfg[key] as string}
              onChange={e => update(key, e.target.value)}
              placeholder={placeholder}
            />
          </div>
        ))}
        <button className="btn btn-primary" onClick={handleSave} disabled={saving}>
          {saving ? t('common.working') : t('common.save')}
        </button>
      </div>
    </div>
  );
}

/* ─── Main Panel ─── */
export default function EnterpriseAuthPanel() {
  const { t } = useT();
  const [activeSubTab, setActiveSubTab] = useState<SubTab>('ldap');

  const tabs: { key: SubTab; label: string }[] = [
    { key: 'ldap', label: t('ldap.title') },
    { key: 'oauth', label: t('oauth.title') },
    { key: 'apikeys', label: t('apikeys.title') },
  ];

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('enterpriseAuth.title')}</h2>
      </div>
      <div className="btn-group" style={{ marginBottom: '16px' }}>
        {tabs.map(tab => (
          <button
            key={tab.key}
            className={`btn btn-sm ${activeSubTab === tab.key ? 'btn-primary' : 'btn-secondary'}`}
            onClick={() => setActiveSubTab(tab.key)}
          >
            {tab.label}
          </button>
        ))}
      </div>
      {activeSubTab === 'ldap' && <LdapTab />}
      {activeSubTab === 'oauth' && <OAuthTab />}
      {activeSubTab === 'apikeys' && <ApiTokensPanel />}
    </div>
  );
}
