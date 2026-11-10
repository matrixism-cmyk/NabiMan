import React, { useState } from 'react';
import { useT } from '../../../shared/i18n';
import { apiRequest } from '../../../shared/api/useApi';
import type { Topology } from './graphModel';

export const DEVICE_KINDS = ['firewall', 'router', 'gateway', 'switch', 'node', 'vip', 'dongle', 'internet', 'storage', 'other'];

interface Props {
  topology: Topology;
  /** The saved servers' hosts, to tie to a device. */
  serverHosts: string[];
  onSaved: (t: Topology) => void;
  onClose: () => void;
}

type Tab = 'devices' | 'links' | 'hosts' | 'json';

/** The network around the servers, edited as three small tables (or as JSON, to paste a whole document). */
export default function TopologyEditor({ topology, serverHosts, onSaved, onClose }: Props) {
  const { t } = useT();
  const [doc, setDoc] = useState<Topology>(() => JSON.parse(JSON.stringify(topology)));
  const [tab, setTab] = useState<Tab>('devices');
  const [json, setJson] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const keys = doc.devices.map((d) => d.key);
  const patch = <K extends keyof Topology>(list: K, i: number, change: Partial<Topology[K][number]>) =>
    setDoc({ ...doc, [list]: (doc[list] as any[]).map((row, n) => (n === i ? { ...row, ...change } : row)) });
  const drop = (list: keyof Topology, i: number) => setDoc({ ...doc, [list]: (doc[list] as any[]).filter((_, n) => n !== i) });
  // Removing a device takes its links and ties with it.
  const dropDevice = (i: number) => {
    const key = doc.devices[i].key;
    setDoc({ devices: doc.devices.filter((_, n) => n !== i), links: doc.links.filter((l) => l.from !== key && l.to !== key), hosts: doc.hosts.filter((h) => h.device !== key) });
  };
  const open = (next: Tab) => {
    if (next === 'json') setJson(JSON.stringify(doc, null, 2));
    if (tab === 'json' && next !== 'json') {
      try { setDoc(JSON.parse(json)); setError(''); } catch { setError(t('topology.badJson')); return; }
    }
    setTab(next);
  };
  const save = async () => {
    let body = doc;
    if (tab === 'json') {
      try { body = JSON.parse(json); } catch { setError(t('topology.badJson')); return; }
    }
    setBusy(true);
    setError('');
    try {
      const res: any = await apiRequest('/api/terminal/topology', { method: 'PUT', body: JSON.stringify(body) });
      if (res?.success) { onSaved(res.data); onClose(); } else setError(res?.message || res?.error || t('topology.saveFailed'));
    } catch (e) {
      setError(e instanceof Error ? e.message : t('topology.saveFailed'));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content topology-editor" role="dialog" aria-label={t('topology.title')} onClick={(e) => e.stopPropagation()}>
        <h3>{t('topology.title')}</h3>
        <p className="text-secondary topology-editor-note">{t('topology.note')}</p>
        <div className="btn-group" role="tablist">
          {(['devices', 'links', 'hosts', 'json'] as Tab[]).map((x) => (
            <button key={x} type="button" role="tab" aria-selected={tab === x} className={`btn btn-sm ${tab === x ? 'btn-primary' : 'btn-secondary'}`} onClick={() => open(x)}>
              {t(`topology.tab.${x}`)}{x === 'devices' ? ` ${doc.devices.length}` : x === 'links' ? ` ${doc.links.length}` : x === 'hosts' ? ` ${doc.hosts.length}` : ''}
            </button>
          ))}
        </div>
        <div className="topology-editor-body">
          {tab === 'devices' && (
            <table className="topology-table">
              <thead><tr><th>{t('topology.key')}</th><th>{t('topology.name')}</th><th>{t('topology.kind')}</th><th>{t('topology.address')}</th><th>{t('topology.memo')}</th><th /></tr></thead>
              <tbody>
                {doc.devices.map((d, i) => (
                  <tr key={i}>
                    <td><input aria-label={t('topology.key')} value={d.key} onChange={(e) => patch('devices', i, { key: e.target.value.trim() })} /></td>
                    <td><input aria-label={t('topology.name')} value={d.name} onChange={(e) => patch('devices', i, { name: e.target.value })} /></td>
                    <td><select aria-label={t('topology.kind')} value={d.kind} onChange={(e) => patch('devices', i, { kind: e.target.value })}>{DEVICE_KINDS.map((k) => <option key={k} value={k}>{t(`topology.kind.${k}`)}</option>)}</select></td>
                    <td><input aria-label={t('topology.address')} value={d.address} onChange={(e) => patch('devices', i, { address: e.target.value })} /></td>
                    <td><input aria-label={t('topology.memo')} value={d.note} onChange={(e) => patch('devices', i, { note: e.target.value })} /></td>
                    <td><button type="button" className="btn btn-danger btn-sm" onClick={() => dropDevice(i)} aria-label={t('topology.remove')}>✕</button></td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {tab === 'links' && (
            <table className="topology-table">
              <thead><tr><th>{t('topology.from')}</th><th>{t('topology.to')}</th><th>{t('topology.label')}</th><th /></tr></thead>
              <tbody>
                {doc.links.map((l, i) => (
                  <tr key={i}>
                    <td><select aria-label={t('topology.from')} value={l.from} onChange={(e) => patch('links', i, { from: e.target.value })}>{keys.map((k) => <option key={k}>{k}</option>)}</select></td>
                    <td><select aria-label={t('topology.to')} value={l.to} onChange={(e) => patch('links', i, { to: e.target.value })}>{keys.map((k) => <option key={k}>{k}</option>)}</select></td>
                    <td><input aria-label={t('topology.label')} value={l.label} onChange={(e) => patch('links', i, { label: e.target.value })} /></td>
                    <td><button type="button" className="btn btn-danger btn-sm" onClick={() => drop('links', i)} aria-label={t('topology.remove')}>✕</button></td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {tab === 'hosts' && (
            <table className="topology-table">
              <thead><tr><th>{t('topology.serverHost')}</th><th>{t('topology.behind')}</th><th /></tr></thead>
              <tbody>
                {doc.hosts.map((h, i) => (
                  <tr key={i}>
                    <td><input aria-label={t('topology.serverHost')} list="topology-hosts" value={h.server_host} onChange={(e) => patch('hosts', i, { server_host: e.target.value.trim() })} /></td>
                    <td><select aria-label={t('topology.behind')} value={h.device} onChange={(e) => patch('hosts', i, { device: e.target.value })}>{keys.map((k) => <option key={k}>{k}</option>)}</select></td>
                    <td><button type="button" className="btn btn-danger btn-sm" onClick={() => drop('hosts', i)} aria-label={t('topology.remove')}>✕</button></td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {tab === 'json' && <textarea className="topology-json" aria-label="JSON" value={json} onChange={(e) => setJson(e.target.value)} spellCheck={false} />}
          <datalist id="topology-hosts">{serverHosts.map((h) => <option key={h} value={h} />)}</datalist>
        </div>
        {error && <div role="alert" className="message">{error}</div>}
        <div className="topology-editor-actions">
          {tab === 'devices' && <button type="button" className="btn btn-secondary btn-sm" onClick={() => setDoc({ ...doc, devices: [...doc.devices, { key: `dev-${doc.devices.length + 1}`, name: '', kind: 'other', address: '', note: '' }] })}>＋ {t('topology.addDevice')}</button>}
          {tab === 'links' && <button type="button" className="btn btn-secondary btn-sm" disabled={keys.length < 2} onClick={() => setDoc({ ...doc, links: [...doc.links, { from: keys[0], to: keys[1], label: '' }] })}>＋ {t('topology.addLink')}</button>}
          {tab === 'hosts' && <button type="button" className="btn btn-secondary btn-sm" disabled={!keys.length} onClick={() => setDoc({ ...doc, hosts: [...doc.hosts, { device: keys[0], server_host: '' }] })}>＋ {t('topology.addHost')}</button>}
          <span className="topology-editor-spacer" />
          <button type="button" className="btn btn-secondary btn-sm" onClick={onClose}>{t('topology.cancel')}</button>
          <button type="button" className="btn btn-primary btn-sm" disabled={busy} onClick={() => void save()}>{t('topology.save')}</button>
        </div>
      </div>
    </div>
  );
}
