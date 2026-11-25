import React from 'react';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { useApi } from '../../shared/api/useApi';
import RackPanel from './RackPanel';
import { checkedBy, latency } from './rackFormat';
import type { DeviceStatus, RackDevice, RackLayout, RackStatus } from './rackTypes';

jest.mock('../../shared/api/useApi', () => ({ useApi: jest.fn() }));

const device = (over: Partial<RackDevice> & { id: string }): RackDevice => ({
  label: over.id, type: 'switch', u: null, h: 1, u_confirmed: true, node: null, node_candidates: [], ip: null, bmc: null,
  gpu: null, cpu: null, arch: null, desc: null, order_hint: null, check: 'ping', external: false, ...over,
});

/** Devices that are no node, one for each thing the page can say about them. */
const layout: RackLayout = {
  version: 'v-reach', title: 'Room', sources: [], notes: null, power: [], overlaps: [],
  racks: [
    { id: 'R1', name: 'RACK #1', role: null, units: 42, external: false, devices: [
      device({ id: 'nexus', label: 'Nexus', u: 40, ip: '172.20.26.1' }),
      device({ id: 'core', label: 'Core DU', type: '5g', u: 39, ip: '172.20.26.5' }),
      device({ id: 'fw', label: 'Firewall', type: 'firewall', u: 38, h: 2, ip: '203.0.113.9', check: 'tcp:3344' }),
      device({ id: 'mini', label: 'Mini PC', type: '5g', u: 36, ip: '172.20.26.102' }),
      device({ id: 'aruba', label: 'Aruba', u: 35 }),
      device({ id: 'pdu', label: 'PDU', type: 'other', u: 34, ip: '172.20.26.9', check: 'none' }),
      device({ id: 'ap', label: 'AP', u: 33, ip: '172.40.0.10' }),
      device({ id: 'srv', label: 'Server', type: 'server', u: 31, h: 2, node: 'mec-cp01', ip: '172.20.26.201' }),
    ] },
    { id: 'R2', name: 'RACK #2', role: 'not ours', units: 42, external: true, devices: [
      device({ id: 'theirs', label: 'Their box', type: 'external', u: 40, h: 2, ip: '172.16.0.10', external: true }),
    ] },
  ],
  nodes: [{ name: 'mec-cp01', ip: '172.20.26.201', role: null, model: null, cpu: null, mem_gi: null, gpu: null, arch: null }],
};

const dev = (over: Partial<DeviceStatus>): DeviceStatus => ({ level: 'none', nodes: [], external: false, reason: null, latency_ms: null, checked_by: null, checked_at: null, ...over });

const status: RackStatus = {
  collected_at: '2026-11-25T12:00:00Z', source: 'ok', message: null, layout_version: 'v-reach',
  nodes: { 'mec-cp01': { level: 'ok', reasons: [], ready: true, cpu_pct: 10, mem_pct: 20, disk_pct: 30, temp_c: 40, gpus: [] } },
  devices: {
    nexus: dev({ level: 'ok', latency_ms: 0.61, checked_by: 'ping', checked_at: '2026-11-25T12:00:05Z' }),
    core: dev({ level: 'ok', latency_ms: 0.42, checked_by: 'prometheus-icmp', checked_at: '2026-11-25T12:00:00Z' }),
    fw: dev({ level: 'ok', latency_ms: 11.46, checked_by: 'tcp:3344', checked_at: '2026-11-25T12:00:05Z' }),
    mini: dev({ level: 'noreply', checked_by: 'ping', checked_at: '2026-11-25T12:00:05Z' }),
    aruba: dev({ reason: 'no_address' }),
    pdu: dev({ reason: 'check_none' }),
    ap: dev({ level: 'unknown', reason: 'ping_denied', checked_by: 'ping', checked_at: '2026-11-25T12:00:05Z' }),
    srv: dev({ level: 'ok', nodes: ['mec-cp01'] }),
    theirs: dev({ level: 'noreply', external: true, checked_by: 'ping', checked_at: '2026-11-25T12:00:05Z' }),
  },
  summary: { nodes_total: 1, nodes_ready: 1, gpus: 0, warnings: 0, down: 0, max_temp_c: 40, probed: 6, reachable: 3, noreply: 1, external_noreply: 1, over_limit: 0 },
  thresholds: { temp_c: 80, memory_pct: 90, disk_pct: 90 },
};

function show(with_status: RackStatus = status) {
  (useApi as jest.Mock).mockImplementation((endpoint: string) => (endpoint === '/api/rack/layout'
    ? { data: { configured: true, layout, error: null }, loading: false, error: null, refetch: jest.fn() }
    : { data: { configured: true, status: with_status, error: null }, loading: false, error: null, refetch: jest.fn() }));
  localStorage.setItem('nabiman_lang', 'en');
  return render(<I18nProvider><RackPanel /></I18nProvider>);
}

const face = (id: string) => document.querySelector(`[data-device="${id}"]`) as HTMLButtonElement;
const words = (id: string) => face(id).querySelector('[data-reach]')?.textContent ?? null;
const open = (id: string) => { fireEvent.click(face(id)); return screen.getByRole('dialog'); };

describe('devices that are not nodes', () => {
  it('say on their face whether they answer, how fast, or what is missing', () => {
    show();
    expect(words('nexus')).toBe('Answering · 0.6ms');
    expect(words('core')).toBe('Answering · 0.4ms');
    expect(words('fw')).toBe('Answering · 11ms');
    expect(words('mini')).toBe('No reply');
    expect(words('aruba')).toBe('No management address');
    expect(words('pdu')).toBe('Not collected');
    expect(words('ap')).toBe('Could not check');
    expect(words('srv')).toBeNull();
    expect(Object.fromEntries(['nexus', 'mini', 'aruba', 'ap', 'theirs'].map((id) => [id, face(id).getAttribute('data-level')])))
      .toEqual({ nexus: 'ok', mini: 'noreply', aruba: 'none', ap: 'unknown', theirs: 'noreply' });
    expect(face('nexus')).toHaveAccessibleName('Nexus, RACK #1 U40, status Answering · 0.6ms');
    expect(face('aruba')).toHaveAccessibleName('Aruba, RACK #1 U35, status No management address');
    // A 1U face has one line: the words sit beside the name. A taller one has a second line.
    expect(face('nexus').querySelector('.rack-device-head [data-reach]')).not.toBeNull();
    expect(face('fw').querySelector('.rack-device-readings [data-reach]')).not.toBeNull();
  });

  it('mark no reply with its own shape, and calmly when the device is not ours', () => {
    show();
    const ours = within(face('mini')).getByRole('img', { name: 'No reply' });
    expect(ours.textContent).toBe('⊘');
    expect(face('mini')).not.toHaveClass('is-calm');
    const theirs = within(face('theirs')).getByRole('img', { name: 'No reply (not ours)' });
    expect(theirs.textContent).toBe('–');
    expect(theirs).toHaveClass('is-calm');
    expect(face('theirs')).toHaveClass('is-calm');
    expect(words('theirs')).toBe('No reply');
    expect(face('theirs').querySelector('[data-reach]')).toHaveClass('is-calm');
    expect(within(face('srv')).getByRole('img', { name: 'OK' }).textContent).toBe('●');
  });

  it('explain every mark in the legend with a shape and words', () => {
    show();
    const legend = screen.getByLabelText('Legend');
    const marks = Array.from(legend.querySelectorAll('.rack-light')).map((el) => `${el.textContent} ${el.parentElement?.textContent?.slice(el.textContent?.length ?? 0)}`);
    expect(marks).toEqual([
      '● OK · answering', '▲ Warning', '✖ Node down (NotReady)', '⊘ No reply', '? Unknown', '○ Not collected · no management address', '– No reply (not ours)',
    ]);
    expect(new Set(marks.map((m) => m[0])).size).toBe(marks.length);
  });

  it('count the ones that answer in the summary, apart from nodes', () => {
    const first = show();
    const tile = (key: string) => document.querySelector(`[data-tile="${key}"]`);
    expect(tile('reach')?.querySelector('.rack-tile-label')?.textContent).toBe('Devices answering');
    expect(tile('reach')?.querySelector('.rack-tile-value')?.textContent).toBe('⊘ 3/6');
    expect(tile('reach')).toHaveClass('tone-down');
    expect(tile('down')).toBeNull();
    expect(tile('warnings')?.querySelector('.rack-tile-value')?.textContent).toBe('0');
    first.unmount();
    // Only somebody else's device is silent: no alarm.
    const second = show({ ...status, summary: { ...status.summary, probed: 5, reachable: 4, noreply: 0, external_noreply: 1 } });
    expect(tile('reach')?.querySelector('.rack-tile-value')?.textContent).toBe('4/5');
    expect(tile('reach')).toHaveClass('tone-plain');
    second.unmount();
    show({ ...status, summary: { ...status.summary, probed: 0, reachable: 0, noreply: 0, external_noreply: 0, over_limit: 3 } });
    expect(tile('reach')).toBeNull();
    expect(screen.getByText(/3 were not checked for an answer/)).toBeInTheDocument();
  });

  it('tell in the details the address, how it was checked, when and how fast', () => {
    show();
    let dialog = open('fw');
    let card = dialog.querySelector('[data-reach-card]') as HTMLElement;
    expect(within(card).getByText('Answering · 11ms')).toBeInTheDocument();
    expect(within(card).getByText('203.0.113.9')).toBeInTheDocument();
    expect(within(card).getByText('TCP connect to port 3344 from the NabiMan server')).toBeInTheDocument();
    expect(within(card).getByText('Checked at').nextElementSibling?.textContent).toMatch(/\d/);
    expect(within(card).getByText('Response time').nextElementSibling?.textContent).toBe('11ms');
    expect(card.querySelector('[data-why]')).toBeNull();
    fireEvent.keyDown(dialog, { key: 'Escape' });

    dialog = open('core');
    expect(within(dialog).getByText('Prometheus ICMP probe')).toBeInTheDocument();
    fireEvent.keyDown(dialog, { key: 'Escape' });

    dialog = open('mini');
    card = dialog.querySelector('[data-reach-card]') as HTMLElement;
    expect(card).toHaveClass('level-noreply');
    expect(within(card).getByText('Ping from the NabiMan server')).toBeInTheDocument();
    expect(within(card).queryByText('Response time')).toBeNull();
    expect(card.querySelector('[data-external]')).toBeNull();
  });

  it('say what is missing when there is nothing to show, and that a silent outsider is not our fault', () => {
    show();
    let dialog = open('aruba');
    expect(within(dialog).getByText('Management address').nextElementSibling?.textContent).toBe('None');
    expect(within(dialog).getByText(/has no management address \(ip\) for this device yet/)).toBeInTheDocument();
    expect(within(dialog).queryByText('Checked by')).toBeNull();
    fireEvent.keyDown(dialog, { key: 'Escape' });
    dialog = open('pdu');
    expect(within(dialog).getByText(/says not to check this device/)).toBeInTheDocument();
    fireEvent.keyDown(dialog, { key: 'Escape' });
    dialog = open('ap');
    expect(within(dialog).getByText(/is not allowed to send a ping/)).toBeInTheDocument();
    fireEvent.keyDown(dialog, { key: 'Escape' });
    dialog = open('theirs');
    const card = dialog.querySelector('[data-reach-card]') as HTMLElement;
    expect(card).toHaveClass('is-calm');
    expect(within(card).getByText(/Not one of our machines/)).toBeInTheDocument();
    fireEvent.keyDown(dialog, { key: 'Escape' });
    // A node's device keeps its node card and has no reachability card.
    dialog = open('srv');
    expect(dialog.querySelector('[data-reach-card]')).toBeNull();
    expect(dialog.querySelector('.rack-node-card[data-node="mec-cp01"]')).not.toBeNull();
  });

  it('format latency and the way of checking', () => {
    const t = (key: string, params?: Record<string, string | number>) => `${key}${params ? `:${params.port}` : ''}`;
    expect([latency(0.04), latency(0.61), latency(9.96), latency(12.4), latency(250), latency(null)]).toEqual(['<0.1ms', '0.6ms', '10.0ms', '12ms', '250ms', '']);
    expect([checkedBy(t, 'ping'), checkedBy(t, 'tcp:22'), checkedBy(t, 'prometheus-tcp'), checkedBy(t, 'new-way'), checkedBy(t, null)])
      .toEqual(['rack.by.ping', 'rack.by.tcp:22', 'rack.by.prometheus-tcp', 'new-way', '']);
  });
});
