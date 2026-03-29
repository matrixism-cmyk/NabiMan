import React, { useState, useEffect, useRef } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { useT } from '../i18n';
import { useSortable } from '../hooks/useSortable';

interface IpBlockStatus {
  backend: string;
  active: boolean;
  jails: JailInfo[];
}

interface JailInfo {
  name: string;
  enabled: boolean;
  banned_count: number;
  filter: string;
  max_retry: number;
  ban_time: number;
  find_time: number;
}

interface BannedIp {
  ip: string;
  jail: string;
  ban_time: string;
  expires: string;
}

interface BlockLogEntry {
  timestamp: string;
  action: string;
  ip: string;
  jail: string;
}

export default function IpBlockPanel() {
  const { t } = useT();
  const { data: status, loading, error, refetch: refetchStatus } =
    useApi<IpBlockStatus>('/api/ipblock/status', 10000);
  const { data: banned, refetch: refetchBanned } =
    useApi<BannedIp[]>('/api/ipblock/banned', 10000);
  const { data: log, refetch: refetchLog } =
    useApi<BlockLogEntry[]>('/api/ipblock/log');

  const [banIp, setBanIp] = useState('');
  const [banJail, setBanJail] = useState('');
  const [banDuration, setBanDuration] = useState('3600');
  const [msg, setMsg] = useState('');
  const [working, setWorking] = useState(false);

  // Set default jail when jails load
  const jailsLoaded = useRef(false);
  useEffect(() => {
    if (status?.jails && status.jails.length > 0 && !jailsLoaded.current) {
      setBanJail(status.jails[0].name);
      jailsLoaded.current = true;
    }
  }, [status]);

  const handleRefresh = () => {
    refetchStatus();
    refetchBanned();
    refetchLog();
  };

  const handleUnban = async (ip: string, jail: string) => {
    setWorking(true);
    setMsg('');
    const res = await apiPost<string>('/api/ipblock/unban', { ip, jail });
    setMsg(res.data || res.message);
    setWorking(false);
    refetchBanned();
    refetchLog();
  };

  const handleBan = async () => {
    if (!banIp || !banJail) return;
    setWorking(true);
    setMsg('');
    const duration = parseInt(banDuration, 10) || 3600;
    const res = await apiPost<string>('/api/ipblock/ban', {
      ip: banIp, jail: banJail, duration,
    });
    setMsg(res.data || res.message);
    setWorking(false);
    if (res.success) { setBanIp(''); }
    refetchBanned();
    refetchLog();
  };

  const { sorted: sortedBanned, toggle, indicator } = useSortable(banned || []);
  const S = (key: string, label: string) => (
    <th className="sortable" onClick={() => toggle(key)}>{label}{indicator(key)}</th>
  );

  const actionBadge = (action: string) => {
    const cls = action === 'ban' ? 'down' : 'up';
    return <span className={`status-badge ${cls}`}>{action.toUpperCase()}</span>;
  };

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;
  if (!status) return null;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('ipblock.title')}</h2>
        <button className="btn btn-secondary btn-sm" onClick={handleRefresh}>
          {t('ipblock.refresh')}
        </button>
      </div>
      {msg && <div className="message" onClick={() => setMsg('')}>{msg}</div>}

      {/* Status Overview */}
      <div className="info-grid">
        <div className="info-item">
          <span className="info-label">{t('ipblock.backend')}</span>
          <span className="info-value"><code>{status.backend}</code></span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('common.status')}</span>
          <span className="info-value">
            <span className={`status-badge ${status.active ? 'up' : 'down'}`}>
              {status.active ? t('ipblock.active') : t('ipblock.inactive')}
            </span>
          </span>
        </div>
      </div>

      {/* Jail Summary */}
      {status.jails.length > 0 && (
        <>
          <h3>{t('ipblock.jails')}</h3>
          <table className="data-table">
            <thead>
              <tr>
                <th>{t('ipblock.jailName')}</th>
                <th>{t('common.status')}</th>
                <th>{t('ipblock.bannedCount')}</th>
                <th>{t('ipblock.filter')}</th>
                <th>{t('ipblock.maxRetry')}</th>
                <th>{t('ipblock.banTime')}</th>
                <th>{t('ipblock.findTime')}</th>
              </tr>
            </thead>
            <tbody>
              {status.jails.map((jail, i) => (
                <tr key={i}>
                  <td><strong>{jail.name}</strong></td>
                  <td>
                    <span className={`status-badge ${jail.enabled ? 'up' : 'down'}`}>
                      {jail.enabled ? t('ipblock.enabled') : t('ipblock.disabled')}
                    </span>
                  </td>
                  <td>{jail.banned_count}</td>
                  <td><code>{jail.filter}</code></td>
                  <td>{jail.max_retry}</td>
                  <td>{jail.ban_time}s</td>
                  <td>{jail.find_time}s</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      {/* Banned IPs */}
      <h3>{t('ipblock.bannedIps')}</h3>
      {sortedBanned.length > 0 ? (
        <table className="data-table">
          <thead>
            <tr>
              {S('ip', t('ipblock.ip'))}
              {S('jail', t('ipblock.jail'))}
              {S('ban_time', t('ipblock.banTimeCol'))}
              {S('expires', t('ipblock.expires'))}
              <th>{t('common.actions')}</th>
            </tr>
          </thead>
          <tbody>
            {sortedBanned.map((b, i) => (
              <tr key={i}>
                <td><code>{b.ip}</code></td>
                <td>{b.jail}</td>
                <td>{b.ban_time}</td>
                <td>{b.expires}</td>
                <td>
                  <button
                    className="btn btn-sm btn-danger"
                    onClick={() => handleUnban(b.ip, b.jail)}
                    disabled={working}
                  >
                    {t('ipblock.unban')}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <p className="text-secondary">{t('ipblock.noBanned')}</p>
      )}

      {/* Manual Ban Form */}
      <h3>{t('ipblock.manualBan')}</h3>
      <div className="inline-form">
        <div className="form-row">
          <label>{t('ipblock.ip')}</label>
          <input
            className="filter-input"
            value={banIp}
            onChange={e => setBanIp(e.target.value)}
            placeholder="192.168.1.100"
          />
        </div>
        <div className="form-row">
          <label>{t('ipblock.jail')}</label>
          <select
            className="select-input"
            value={banJail}
            onChange={e => setBanJail(e.target.value)}
          >
            {status.jails.map((jail, i) => (
              <option key={i} value={jail.name}>{jail.name}</option>
            ))}
          </select>
        </div>
        <div className="form-row">
          <label>{t('ipblock.duration')}</label>
          <input
            className="filter-input"
            type="number"
            value={banDuration}
            onChange={e => setBanDuration(e.target.value)}
            placeholder="3600"
          />
        </div>
        <button className="btn btn-primary" onClick={handleBan} disabled={working || !banIp}>
          {working ? t('common.working') : t('ipblock.ban')}
        </button>
      </div>

      {/* Block Log */}
      <h3>{t('ipblock.log')}</h3>
      {log && log.length > 0 ? (
        <table className="data-table">
          <thead>
            <tr>
              <th>{t('ipblock.timestamp')}</th>
              <th>{t('ipblock.action')}</th>
              <th>{t('ipblock.ip')}</th>
              <th>{t('ipblock.jail')}</th>
            </tr>
          </thead>
          <tbody>
            {log.slice(0, 50).map((entry, i) => (
              <tr key={i}>
                <td><code>{entry.timestamp}</code></td>
                <td>{actionBadge(entry.action)}</td>
                <td><code>{entry.ip}</code></td>
                <td>{entry.jail}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <p className="text-secondary">{t('ipblock.noLog')}</p>
      )}
    </div>
  );
}
