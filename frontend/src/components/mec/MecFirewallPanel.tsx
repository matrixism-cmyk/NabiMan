import React, { useState } from 'react';
import { useMecList, mecDelete } from '../../hooks/mec/useMecApi';
import { NatRule, PublicIp } from '../../types/mec';
import FirewallNatForm from './FirewallNatForm';
import {
  ErrorBanner,
  MetricCard,
  MetricGrid,
  PanelLayout,
  Section,
  SortableTable,
  StatusBadge,
  Toolbar,
} from './common';

function sourceDisplay(s: NatRule['source']): string {
  if ('any' in s) return 'any';
  return s.groups.join(', ');
}

/// Find NAT rule(s) that map to this public IP (destination contains IP/32).
function findRulesForIp(rules: NatRule[], ip: string): NatRule[] {
  return rules.filter((r) =>
    r.destination.some((d) => d === ip || d === `${ip}/32`),
  );
}

export default function MecFirewallPanel() {
  const rules = useMecList<NatRule>('/api/mec/v1/firewall/nat-rules', 60_000);
  const ips = useMecList<PublicIp>('/api/mec/v1/firewall/public-ips', 60_000);
  const [showAdd, setShowAdd] = useState(false);
  const [prefillIp, setPrefillIp] = useState<string | null>(null);
  const [natFilter, setNatFilter] = useState('');
  const [ipFilter, setIpFilter] = useState('');
  const [err, setErr] = useState<string | null>(null);
  const [busyIp, setBusyIp] = useState<string | null>(null);

  const natRules = rules.data || [];
  const publicIps = ips.data || [];
  const activeNat = natRules.filter((r) => r.enabled).length;
  const availableIp = publicIps.filter((p) => p.status === 'available').length;
  const assignedIp = publicIps.filter(
    (p) => p.status === 'assigned' || p.status === 'system_reserved',
  ).length;

  const refresh = () => {
    rules.refetch();
    ips.refetch();
  };

  const onAssignClick = (ip: string) => {
    setPrefillIp(ip);
    setShowAdd(true);
  };

  const onReleaseClick = async (ip: string) => {
    const matched = findRulesForIp(natRules, ip);
    if (matched.length === 0) {
      setErr(`공인 IP ${ip} 에 매핑된 NAT 규칙을 찾지 못했습니다.`);
      return;
    }
    const summary = [
      `⚠️ 공인 IP '${ip}' 할당 해제`,
      '',
      `연결된 NAT 규칙 ${matched.length}건이 삭제됩니다:`,
      ...matched.map((r) => `  • ${r.id} (${r.label || '-'}) → ${r.translated_to}`),
      '',
      'proxy-arp 도 같이 해제됩니다 (AXGATE add_nat_rule delete 흐름).',
      '',
      '확인을 위해 IP 를 다시 입력하세요:',
    ].join('\n');
    const typed = window.prompt(summary, '');
    if (typed !== ip) {
      if (typed !== null) setErr('입력 IP 가 일치하지 않아 해제 취소.');
      return;
    }
    setBusyIp(ip);
    setErr(null);
    try {
      for (const r of matched) {
        await mecDelete(`/api/mec/v1/firewall/nat-rules/${r.id}`, r.id);
      }
      await refresh();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyIp(null);
    }
  };

  const onDeleteRule = async (id: string) => {
    const typed = window.prompt(
      `⚠️ NAT 규칙 '${id}' 삭제 (외부 접근 끊길 수 있음). ID 재입력:`,
      '',
    );
    if (typed !== id) {
      if (typed !== null) setErr('입력 ID 가 일치하지 않아 삭제 취소.');
      return;
    }
    try {
      await mecDelete(`/api/mec/v1/firewall/nat-rules/${id}`, id);
      await refresh();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <PanelLayout
      title="AXGATE 방화벽"
      subtitle={`NAT ${activeNat}/${natRules.length} 활성 · 공인 IP 할당 ${assignedIp} · 가용 ${availableIp}`}
      actions={
        <>
          <button className="btn btn-secondary" onClick={refresh}>
            새로고침
          </button>
          <button
            className="btn btn-primary"
            onClick={() => {
              setPrefillIp(null);
              setShowAdd((v) => !v);
            }}
          >
            {showAdd ? '취소' : '+ NAT 규칙'}
          </button>
        </>
      }
    >
      <ErrorBanner error={rules.error || ips.error || err || undefined} />

      <MetricGrid>
        <MetricCard
          label="NAT 규칙"
          value={activeNat}
          unit={`/ ${natRules.length}`}
          helper={`${natRules.length - activeNat}개 비활성`}
          tone="info"
        />
        <MetricCard
          label="공인 IP 할당"
          value={assignedIp}
          unit={`/ ${publicIps.length}`}
          helper={`${availableIp}개 가용`}
          tone={availableIp === 0 ? 'warning' : 'info'}
        />
      </MetricGrid>

      {showAdd && (
        <FirewallNatForm
          publicIps={publicIps}
          prefillPublicIp={prefillIp}
          onCancel={() => {
            setShowAdd(false);
            setPrefillIp(null);
          }}
          onCreated={async () => {
            setShowAdd(false);
            setPrefillIp(null);
            await refresh();
          }}
        />
      )}

      <Section title="공인 IP" marginTop="24px">
        <Toolbar>
          <input
            placeholder="필터 (IP, 상태)"
            value={ipFilter}
            onChange={(e) => setIpFilter(e.target.value)}
            style={{ padding: '6px 10px', minWidth: '240px' }}
          />
        </Toolbar>
        <SortableTable<PublicIp>
          data={publicIps}
          filter={ipFilter}
          defaultSortKey="ip"
          rowKey={(p) => p.ip}
          emptyMessage="공인 IP 정보가 없습니다."
          columns={[
            {
              key: 'ip',
              header: 'IP',
              accessor: (p) => p.ip,
              render: (p) => <code>{p.ip}</code>,
              sortable: true,
            },
            {
              key: 'status',
              header: '상태',
              accessor: (p) => p.status,
              render: (p) => <StatusBadge tone={p.status}>{p.status}</StatusBadge>,
              width: '140px',
              sortable: true,
            },
            {
              key: 'assigned',
              header: '용도',
              accessor: (p) => p.assigned_to || '',
              render: (p) => {
                const matched = findRulesForIp(natRules, p.ip);
                if (matched.length === 0) {
                  return (
                    <span style={{ color: 'var(--text-secondary)' }}>
                      {p.assigned_to || '-'}
                    </span>
                  );
                }
                return (
                  <span style={{ fontSize: '12px' }}>
                    {matched
                      .map(
                        (r) =>
                          `${r.label || r.id} → ${r.translated_to.replace('/32', '')}`,
                      )
                      .join(', ')}
                  </span>
                );
              },
              sortable: true,
            },
            {
              key: 'proxy_arp',
              header: 'proxy-arp',
              accessor: (p) => p.proxy_arp_enabled,
              render: (p) => (
                <StatusBadge tone={p.proxy_arp_enabled ? 'success' : 'neutral'}>
                  {p.proxy_arp_enabled ? '✓' : '-'}
                </StatusBadge>
              ),
              width: '100px',
              sortable: true,
            },
            {
              key: 'actions',
              header: '',
              accessor: () => '',
              sortable: false,
              width: '120px',
              render: (p) => {
                if (p.status === 'system_reserved') {
                  return (
                    <span
                      style={{ color: 'var(--text-secondary)', fontSize: '12px' }}
                    >
                      시스템 예약
                    </span>
                  );
                }
                if (p.status === 'available') {
                  return (
                    <button
                      className="btn btn-primary btn-small"
                      onClick={() => onAssignClick(p.ip)}
                    >
                      할당
                    </button>
                  );
                }
                return (
                  <button
                    className="btn btn-danger btn-small"
                    disabled={busyIp === p.ip}
                    onClick={() => onReleaseClick(p.ip)}
                  >
                    {busyIp === p.ip ? '해제 중...' : '해제'}
                  </button>
                );
              },
            },
          ]}
        />
      </Section>

      <Section title="NAT 규칙" marginTop="24px">
        <Toolbar>
          <input
            placeholder="필터 (ID, label, IP, zone)"
            value={natFilter}
            onChange={(e) => setNatFilter(e.target.value)}
            style={{ padding: '6px 10px', minWidth: '240px' }}
          />
        </Toolbar>
        <SortableTable<NatRule>
          data={natRules}
          filter={natFilter}
          defaultSortKey="id"
          rowKey={(r) => `${r.from_zone}-${r.to_zone}-${r.id}`}
          emptyMessage="NAT 규칙이 없습니다."
          columns={[
            {
              key: 'id',
              header: 'ID',
              accessor: (r) => r.id,
              render: (r) => <code>{r.id}</code>,
              width: '90px',
              sortable: true,
            },
            {
              key: 'zone',
              header: 'Zone',
              accessor: (r) => `${r.from_zone}→${r.to_zone}`,
              render: (r) => (
                <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
                  {r.from_zone} → {r.to_zone}
                </span>
              ),
              sortable: true,
            },
            {
              key: 'type',
              header: 'Type',
              accessor: (r) => r.rule_type,
              render: (r) => (
                <StatusBadge tone={r.rule_type === 'dnat' ? 'info' : 'neutral'}>
                  {r.rule_type}
                </StatusBadge>
              ),
              width: '80px',
              sortable: true,
            },
            {
              key: 'label',
              header: 'Label',
              accessor: (r) => r.label || '',
              render: (r) =>
                r.label || (
                  <span style={{ color: 'var(--text-secondary)' }}>-</span>
                ),
              sortable: true,
            },
            {
              key: 'dst',
              header: '공인 IP',
              accessor: (r) => r.destination.join(','),
              render: (r) => r.destination.join(', ') || '-',
              sortable: true,
            },
            {
              key: 'src',
              header: 'Source',
              accessor: (r) => sourceDisplay(r.source),
              render: (r) => (
                <span style={{ fontSize: '11px' }}>{sourceDisplay(r.source)}</span>
              ),
              sortable: true,
            },
            {
              key: 'to',
              header: 'Translated',
              accessor: (r) => r.translated_to,
              render: (r) => <code>{r.translated_to}</code>,
              sortable: true,
            },
            {
              key: 'service',
              header: 'Service',
              accessor: (r) => r.service_groups.join(','),
              render: (r) => r.service_groups.join(', ') || '-',
              sortable: true,
            },
            {
              key: 'enabled',
              header: 'Enabled',
              accessor: (r) => r.enabled,
              render: (r) => (
                <StatusBadge tone={r.enabled}>
                  {r.enabled ? 'active' : 'disabled'}
                </StatusBadge>
              ),
              width: '100px',
              sortable: true,
            },
            {
              key: 'actions',
              header: '',
              accessor: () => '',
              sortable: false,
              width: '80px',
              render: (r) => (
                <button
                  className="btn btn-danger btn-small"
                  onClick={() => onDeleteRule(r.id)}
                >
                  삭제
                </button>
              ),
            },
          ]}
        />
      </Section>
    </PanelLayout>
  );
}
