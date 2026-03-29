import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { useT } from '../i18n';

interface LicenseStatus {
  active: boolean;
  tier: string;
  holder: string;
  expires_at: string | null;
  days_remaining: number | null;
  is_trial: boolean;
  trial_days_remaining: number | null;
  features: string[];
  max_servers: number | null;
}

export default function LicensePanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<LicenseStatus>('/api/license');
  const { data: features } = useApi<string[]>('/api/license/features');
  const [key, setKey] = useState('');
  const [msg, setMsg] = useState('');
  const [activating, setActivating] = useState(false);

  const handleActivate = async () => {
    if (!key.trim()) return;
    setActivating(true);
    setMsg('');
    const res = await apiPost<string>('/api/license/activate', { key: key.trim() });
    setMsg(res.data || res.message);
    setActivating(false);
    if (res.success) { setKey(''); refetch(); }
  };

  const tierBadgeClass = (tier: string) => {
    switch (tier.toLowerCase()) {
      case 'pro': return 'status-badge up';
      case 'enterprise': return 'status-badge up';
      case 'free': default: return 'status-badge down';
    }
  };

  const tierBadgeStyle = (tier: string): React.CSSProperties => {
    switch (tier.toLowerCase()) {
      case 'pro': return { backgroundColor: '#3b82f6', color: '#fff' };
      case 'enterprise': return { backgroundColor: '#f59e0b', color: '#fff' };
      default: return {};
    }
  };

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;
  if (!data) return null;

  const trialMaxDays = 30;
  const trialProgress = data.trial_days_remaining != null
    ? Math.max(0, Math.min(100, ((trialMaxDays - data.trial_days_remaining) / trialMaxDays) * 100))
    : 0;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('license.title')}</h2>
        <button className="btn btn-secondary btn-sm" onClick={refetch}>
          {t('license.refresh')}
        </button>
      </div>
      {msg && <div className="message" onClick={() => setMsg('')}>{msg}</div>}

      {/* Status Card */}
      <div className="info-grid">
        <div className="info-item">
          <span className="info-label">{t('license.tier')}</span>
          <span className="info-value">
            <span className={tierBadgeClass(data.tier)} style={tierBadgeStyle(data.tier)}>
              {data.tier.toUpperCase()}
            </span>
          </span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('common.status')}</span>
          <span className="info-value">
            <span className={`status-badge ${data.active ? 'up' : 'down'}`}>
              {data.active ? t('license.active') : t('license.inactive')}
            </span>
          </span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('license.holder')}</span>
          <span className="info-value">{data.holder || '-'}</span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('license.expires')}</span>
          <span className="info-value">
            {data.expires_at ? (
              <span className={data.days_remaining != null && data.days_remaining < 30 ? 'text-danger' : ''}>
                {data.expires_at}
                {data.days_remaining != null && ` (${data.days_remaining}${t('license.daysLeft')})`}
              </span>
            ) : t('license.noExpiry')}
          </span>
        </div>
        {data.max_servers != null && (
          <div className="info-item">
            <span className="info-label">{t('license.maxServers')}</span>
            <span className="info-value">{data.max_servers}</span>
          </div>
        )}
      </div>

      {/* Trial Info */}
      {data.is_trial && data.trial_days_remaining != null && (
        <>
          <h3>{t('license.trial')}</h3>
          <div className="info-grid">
            <div className="info-item" style={{ gridColumn: '1 / -1' }}>
              <span className="info-label">
                {t('license.trialRemaining').replace('{days}', String(data.trial_days_remaining))}
              </span>
              <div className="progress-bar" style={{ width: '100%', marginTop: '4px' }}>
                <div
                  className="progress-fill"
                  style={{
                    width: `${trialProgress}%`,
                    backgroundColor: data.trial_days_remaining < 7 ? '#e74c3c' : '#f39c12',
                  }}
                />
              </div>
            </div>
          </div>
        </>
      )}

      {/* Features List */}
      <h3>{t('license.features')}</h3>
      {features && features.length > 0 ? (
        <div className="info-grid">
          {features.map((feat, i) => {
            const enabled = data.features.includes(feat);
            return (
              <div className="info-item" key={i}>
                <span className="info-label">
                  <span className={`status-badge ${enabled ? 'up' : 'down'}`}>
                    {enabled ? t('license.enabled') : t('license.disabled')}
                  </span>
                </span>
                <span className="info-value">{feat}</span>
              </div>
            );
          })}
        </div>
      ) : (
        <div className="info-grid">
          {data.features.map((feat, i) => (
            <div className="info-item" key={i}>
              <span className="info-label">
                <span className="status-badge up">{t('license.enabled')}</span>
              </span>
              <span className="info-value">{feat}</span>
            </div>
          ))}
        </div>
      )}

      {/* Activate Form */}
      <h3>{t('license.activate')}</h3>
      <div className="inline-form">
        <div className="form-row">
          <label>{t('license.key')}</label>
          <textarea
            className="filter-input"
            rows={3}
            value={key}
            onChange={e => setKey(e.target.value)}
            placeholder={t('license.keyPlaceholder')}
            style={{ fontFamily: 'monospace', width: '100%' }}
          />
        </div>
        <button
          className="btn btn-primary"
          onClick={handleActivate}
          disabled={activating || !key.trim()}
        >
          {activating ? t('common.working') : t('license.activateBtn')}
        </button>
      </div>
    </div>
  );
}
