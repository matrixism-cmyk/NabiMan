import React, { useEffect, useState } from 'react';
import { useT } from '../../i18n';
import { mecPost } from '../../hooks/mec/useMecApi';
import { PublicIp } from '../../types/mec';
import { ErrorBanner } from './common';

interface Props {
  publicIps: PublicIp[];
  /** If set, the form is opened prefilled with this public IP (e.g. from row action). */
  prefillPublicIp?: string | null;
  onCreated: () => void | Promise<void>;
  onCancel: () => void;
}

export default function FirewallNatForm({
  publicIps,
  prefillPublicIp,
  onCreated,
  onCancel,
}: Props) {
  const { t } = useT();
  const [form, setForm] = useState({
    label: '',
    public_ip: prefillPublicIp || '',
    custom_public_ip: false,
    private_ip: '',
    protocol: 'tcp' as 'tcp' | 'udp' | 'icmp',
    ports: '22',
    create_proxy_arp: true,
    enable_immediately: true,
  });
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    if (prefillPublicIp) {
      setForm((f) => ({ ...f, public_ip: prefillPublicIp, custom_public_ip: false }));
    }
  }, [prefillPublicIp]);

  const availableIps = publicIps.filter((p) => p.status === 'available');

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.public_ip) {
      setErr(t('mec.fw.formPublicIpRequired'));
      return;
    }
    setBusy(true);
    setErr(null);
    try {
      const res = await mecPost('/api/mec/v1/firewall/nat-rules', {
        label: form.label,
        public_ip: form.public_ip,
        private_ip: form.private_ip,
        protocol: form.protocol,
        ports: form.ports
          .split(',')
          .map((p) => parseInt(p.trim(), 10))
          .filter((p) => !Number.isNaN(p)),
        source: 'any',
        create_proxy_arp: form.create_proxy_arp,
        enable_immediately: form.enable_immediately,
      });
      if (res.error) setErr(res.error.message);
      else await onCreated();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      onSubmit={submit}
      style={{
        background: 'var(--surface)',
        border: '1px solid var(--border)',
        padding: '14px',
        borderRadius: '8px',
        marginTop: '16px',
        color: 'var(--text)',
      }}
    >
      <h3 style={{ marginTop: 0 }}>{t('mec.fw.formTitle')}</h3>
      <ErrorBanner error={err || undefined} />

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))',
          gap: '8px',
        }}
      >
        <label>
          Label
          <input
            required
            value={form.label}
            onChange={(e) => setForm({ ...form, label: e.target.value })}
            placeholder={t('mec.fw.formLabelPlaceholder')}
          />
        </label>

        <div>
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
            }}
          >
            <span>{t('mec.fw.formPublicIp', { count: availableIps.length })}</span>
            <button
              type="button"
              className="btn btn-secondary btn-small"
              onClick={() =>
                setForm({ ...form, custom_public_ip: !form.custom_public_ip, public_ip: '' })
              }
              style={{ padding: '0 6px', fontSize: '11px' }}
            >
              {form.custom_public_ip ? t('mec.fw.formSelectFromList') : t('mec.fw.formEnterManually')}
            </button>
          </div>
          {form.custom_public_ip ? (
            <input
              required
              value={form.public_ip}
              onChange={(e) => setForm({ ...form, public_ip: e.target.value })}
              placeholder="121.147.13.246"
            />
          ) : (
            <select
              required
              value={form.public_ip}
              onChange={(e) => setForm({ ...form, public_ip: e.target.value })}
            >
              <option value="">{t('mec.fw.formSelectAvailableIp')}</option>
              {availableIps.map((p) => (
                <option key={p.ip} value={p.ip}>
                  {p.ip}
                </option>
              ))}
              {availableIps.length === 0 && (
                <option disabled>{t('mec.fw.formNoAvailableIp')}</option>
              )}
            </select>
          )}
        </div>

        <label>
          {t('mec.fw.formPrivateIp')}
          <input
            required
            value={form.private_ip}
            onChange={(e) => setForm({ ...form, private_ip: e.target.value })}
            placeholder="172.20.26.199"
          />
        </label>
        <label>
          {t('mec.fw.formProtocol')}
          <select
            value={form.protocol}
            onChange={(e) =>
              setForm({
                ...form,
                protocol: e.target.value as typeof form.protocol,
              })
            }
          >
            <option value="tcp">TCP</option>
            <option value="udp">UDP</option>
            <option value="icmp">ICMP</option>
          </select>
        </label>
        <label>
          {t('mec.fw.formPorts')}
          <input
            value={form.ports}
            onChange={(e) => setForm({ ...form, ports: e.target.value })}
            placeholder="22,8080"
          />
        </label>
      </div>

      <label style={{ display: 'block', marginTop: '8px' }}>
        <input
          type="checkbox"
          checked={form.create_proxy_arp}
          onChange={(e) =>
            setForm({ ...form, create_proxy_arp: e.target.checked })
          }
        />{' '}
        {t('mec.fw.formProxyArp')}
      </label>
      <label style={{ display: 'block' }}>
        <input
          type="checkbox"
          checked={form.enable_immediately}
          onChange={(e) =>
            setForm({ ...form, enable_immediately: e.target.checked })
          }
        />{' '}
        {t('mec.fw.formEnableImmediately')}
      </label>

      <div style={{ marginTop: '10px', display: 'flex', gap: '8px' }}>
        <button type="submit" className="btn btn-primary" disabled={busy}>
          {busy ? t('mec.fw.formApplying') : t('mec.fw.formAdd')}
        </button>
        <button type="button" className="btn btn-secondary" onClick={onCancel}>
          {t('mec.fw.cancel')}
        </button>
      </div>
    </form>
  );
}
