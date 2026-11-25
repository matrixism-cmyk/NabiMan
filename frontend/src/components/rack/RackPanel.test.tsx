import React from 'react';
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { useApi } from '../../shared/api/useApi';
import RackPanel, { REFRESH_MS } from './RackPanel';
import type { LayoutAnswer, NodeStatus, RackDevice, RackLayout, RackStatus, StatusAnswer } from './rackTypes';

jest.mock('../../shared/api/useApi', () => ({ useApi: jest.fn() }));

const device = (over: Partial<RackDevice> & { id: string }): RackDevice => ({
  label: over.id, type: 'server', u: null, h: 1, u_confirmed: false, node: null, node_candidates: [], ip: null, bmc: null,
  gpu: null, cpu: null, arch: null, desc: null, order_hint: null, ...over,
});

const layout: RackLayout = {
  version: 'v-test', title: 'Test server room', sources: ['site survey', 'cluster facts'], notes: 'a note',
  power: [{ label: 'UPS', value: 'main 50A' }], overlaps: [],
  racks: [
    { id: 'R1', name: 'RACK #1', role: 'core', units: 42, devices: [
      device({ id: 'sw', label: 'Core switch', type: 'switch', u: 38, u_confirmed: true, ip: '10.0.0.1' }),
      device({ id: 'lenovo-1', label: 'SR650 #1', u: 27, h: 2, node_candidates: ['mec-cp01', 'mec-wn02'], desc: 'which node is not known' }),
      device({ id: 'gh', label: 'GH200 box', u: 21, h: 2, node: 'mec-wn03', ip: '10.0.0.204', bmc: '10.0.0.206', gpu: 'GH200 x1', cpu: 72, arch: 'arm64' }),
    ] },
    { id: 'R2', name: 'RACK #2', role: null, units: 42, devices: [
      device({ id: 'late', label: 'Placed later B', type: 'storage', order_hint: 2 }),
      device({ id: 'early', label: 'Placed later A', type: 'external', order_hint: 1 }),
      device({ id: 't4', label: 'T4 box', u: 36, h: 2, node: 'mec-cp02', ip: '10.0.0.199', gpu: 'T4 x2' }),
    ] },
  ],
  nodes: [
    { name: 'mec-cp01', ip: '10.0.0.201', role: 'control-plane', model: 'SR650 V3', cpu: 32, mem_gi: 125, gpu: null, arch: null },
    { name: 'mec-wn02', ip: '10.0.0.202', role: 'worker', model: 'SR650 V3', cpu: 64, mem_gi: 503, gpu: 'A40 x2', arch: null },
    { name: 'mec-wn03', ip: '10.0.0.204', role: null, model: 'GH200 box', cpu: 72, mem_gi: null, gpu: 'GH200 x1', arch: 'arm64' },
    { name: 'mec-cp02', ip: '10.0.0.199', role: null, model: 'T4 box', cpu: null, mem_gi: null, gpu: 'T4 x2', arch: null },
  ],
};

const node = (over: Partial<NodeStatus>): NodeStatus => ({ level: 'ok', reasons: [], ready: true, cpu_pct: 12, mem_pct: 30, disk_pct: 50, temp_c: 55, gpus: [], ...over });

const status: RackStatus = {
  collected_at: '2026-11-25T12:00:00Z', source: 'ok', message: null, layout_version: 'v-test',
  nodes: {
    'mec-cp01': node({}),
    'mec-wn02': node({ level: 'warn', reasons: ['memory'], mem_pct: 93, gpus: [{ index: 0, mig_id: null, mig_profile: null, model: 'NVIDIA A40', util_pct: 35, temp_c: 41, power_w: 70, mem_used_mib: 601, mem_total_mib: 45487 }] }),
    'mec-wn03': node({ temp_c: 70, gpus: [
      { index: 0, mig_id: 1, mig_profile: '3g.47gb', model: 'NVIDIA GH200 480GB', util_pct: null, temp_c: 32, power_w: 125, mem_used_mib: 38689, mem_total_mib: 47615 },
      { index: 0, mig_id: 2, mig_profile: '3g.47gb', model: 'NVIDIA GH200 480GB', util_pct: null, temp_c: 32, power_w: 125, mem_used_mib: 41980, mem_total_mib: 47615 },
    ] }),
    'mec-cp02': node({ level: 'down', ready: false }),
  },
  devices: {
    sw: { level: 'none', nodes: [] }, 'lenovo-1': { level: 'warn', nodes: ['mec-cp01', 'mec-wn02'] }, gh: { level: 'ok', nodes: ['mec-wn03'] },
    late: { level: 'none', nodes: [] }, early: { level: 'none', nodes: [] }, t4: { level: 'down', nodes: ['mec-cp02'] },
  },
  summary: { nodes_total: 4, nodes_ready: 3, gpus: 2, warnings: 1, down: 1, max_temp_c: 70 },
  thresholds: { temp_c: 80, memory_pct: 90, disk_pct: 90 },
};

const refetchLayout = jest.fn(async () => {});
const refetchStatus = jest.fn(async () => {});

function serve(layoutAnswer: LayoutAnswer | null, statusAnswer: StatusAnswer | null, errors: { layout?: string; status?: string } = {}) {
  (useApi as jest.Mock).mockImplementation((endpoint: string) => (endpoint === '/api/rack/layout'
    ? { data: layoutAnswer, loading: false, error: errors.layout ?? null, refetch: refetchLayout }
    : { data: statusAnswer, loading: false, error: errors.status ?? null, refetch: refetchStatus }));
}

function show() {
  localStorage.setItem('nabiman_lang', 'en');
  return render(<I18nProvider><RackPanel /></I18nProvider>);
}

const ok = () => serve({ configured: true, layout, error: null }, { configured: true, status, error: null });
const deviceButton = (id: string) => document.querySelector(`[data-device="${id}"]`) as HTMLButtonElement;

beforeEach(() => { refetchLayout.mockClear(); refetchStatus.mockClear(); });

describe('RackPanel', () => {
  it('draws each rack with its slots numbered from U1 at the bottom and its devices to scale', () => {
    ok();
    show();
    expect(screen.getByRole('heading', { name: 'Test server room' })).toBeInTheDocument();
    const rack1 = screen.getByRole('region', { name: 'RACK #1, 42U rack' });
    const rail = rack1.querySelectorAll('.rack-rail li');
    expect(rail).toHaveLength(42);
    expect(rail[0].textContent).toBe('42');
    expect(rail[41].textContent).toBe('1');
    // u is the top slot: 2U at u=27 starts 15 slots down from the top (U42) and is 2 slots high.
    const lenovo = deviceButton('lenovo-1');
    expect(lenovo.style.getPropertyValue('--top')).toBe('15');
    expect(lenovo.style.getPropertyValue('--h')).toBe('2');
    expect(deviceButton('sw').style.getPropertyValue('--top')).toBe('4');
    expect(within(rack1).getAllByRole('button')).toHaveLength(3);
  });

  it('dashes the outline of positions not confirmed on site and says so', () => {
    ok();
    show();
    expect(deviceButton('sw')).not.toHaveClass('is-unconfirmed');
    expect(deviceButton('lenovo-1')).toHaveClass('is-unconfirmed');
    expect(screen.getByText(/Some devices are placed provisionally/)).toBeInTheDocument();
    expect(screen.getByText('Dashed = position not yet checked on site')).toBeInTheDocument();
  });

  it('lists devices without a slot under their rack, by order_hint, and draws no list where there is none', () => {
    ok();
    show();
    const rack2 = screen.getByRole('region', { name: 'RACK #2, 42U rack' });
    const listed = Array.from(rack2.querySelectorAll('.rack-unplaced .rack-device')).map((el) => el.getAttribute('data-device'));
    expect(listed).toEqual(['early', 'late']);
    expect(within(rack2).getByRole('heading', { name: 'Slot to be confirmed' })).toBeInTheDocument();
    expect(rack2.querySelectorAll('.rack-slots .rack-device')).toHaveLength(1);
    const rack1 = screen.getByRole('region', { name: 'RACK #1, 42U rack' });
    expect(rack1.querySelector('.rack-unplaced')).toBeNull();
  });

  it('lights each device by its level, in words as well as colour', () => {
    ok();
    show();
    const levels = Object.fromEntries(['sw', 'lenovo-1', 'gh', 't4', 'early'].map((id) => [id, deviceButton(id).getAttribute('data-level')]));
    expect(levels).toEqual({ sw: 'none', 'lenovo-1': 'warn', gh: 'ok', t4: 'down', early: 'none' });
    expect(deviceButton('t4')).toHaveAccessibleName('T4 box, RACK #2 U35–36, status Down (NotReady)');
    expect(deviceButton('sw')).toHaveAccessibleName('Core switch, RACK #1 U38, status Not collected');
    expect(within(deviceButton('gh')).getByRole('img', { name: 'OK' }).textContent).toBe('●');
    expect(within(deviceButton('t4')).getAllByRole('img', { name: 'Down (NotReady)' })[0].textContent).toBe('✖');
    expect(deviceButton('gh')).toHaveClass('look-gpu');
    expect(deviceButton('lenovo-1')).toHaveClass('look-server');
  });

  it('shows a server\'s live numbers on its face, and both candidates of an undecided one', () => {
    ok();
    show();
    const gh = deviceButton('gh');
    expect(gh.textContent).toContain('mec-wn03');
    expect(gh.textContent).toMatch(/CPU 12%.*Mem 30%.*70℃.*GPU 1 –/);
    const lenovo = deviceButton('lenovo-1');
    expect(within(lenovo).getByText('Node undecided')).toBeInTheDocument();
    expect(lenovo.textContent).toContain('mec-cp01');
    expect(lenovo.textContent).toContain('mec-wn02');
    // Its own light (the worst candidate) and the warned candidate's.
    expect(within(lenovo).getAllByRole('img', { name: 'Warning' })).toHaveLength(2);
    expect(within(lenovo).getAllByRole('img', { name: 'OK' })).toHaveLength(1);
  });

  it('sums up nodes, GPUs, warnings and the highest temperature', () => {
    ok();
    show();
    const tile = (key: string) => document.querySelector(`[data-tile="${key}"] .rack-tile-value`)?.textContent;
    expect(tile('nodes')).toBe('3/4');
    expect(tile('gpus')).toBe('2');
    expect(tile('warnings')).toBe('▲ 1');
    expect(tile('down')).toBe('✖ 1');
    expect(tile('maxTemp')).toBe('70℃');
    expect(screen.getByText('Layout version v-test')).toBeInTheDocument();
    expect(screen.getByText('2 sources')).toBeInTheDocument();
    expect(screen.getByText('site survey')).toBeInTheDocument();
    expect(screen.getByText('main 50A')).toBeInTheDocument();
    expect(document.querySelector('[data-updated]')?.textContent).toMatch(/^Status as of /);
  });

  it('opens a device\'s details, with every candidate node side by side, and Escape closes them', () => {
    ok();
    show();
    fireEvent.click(deviceButton('lenovo-1'));
    const dialog = screen.getByRole('dialog', { name: /SR650 #1/ });
    expect(within(dialog).getByText('RACK #1 · U26–27 (2U)')).toBeInTheDocument();
    expect(within(dialog).getByText('Node undecided (mec-cp01, mec-wn02)')).toBeInTheDocument();
    expect(within(dialog).getByText(/Slot unconfirmed: this position has not been checked/)).toBeInTheDocument();
    expect(within(dialog).getByText(/This machine is one of the nodes below/)).toBeInTheDocument();
    expect(within(dialog).getByText('which node is not known')).toBeInTheDocument();
    const cards = dialog.querySelectorAll('.rack-node-card');
    expect(Array.from(cards).map((c) => c.getAttribute('data-node'))).toEqual(['mec-cp01', 'mec-wn02']);
    const wn02 = within(cards[1] as HTMLElement);
    expect(wn02.getByText('▲ Memory nearly full')).toBeInTheDocument();
    expect(wn02.getByText('▲ 93%')).toBeInTheDocument();
    expect(wn02.getByText('NVIDIA A40')).toBeInTheDocument();
    expect(wn02.getByText('0.6 / 44 GiB')).toBeInTheDocument();
    expect(wn02.getByText('70W')).toBeInTheDocument();
    expect(within(cards[0] as HTMLElement).getByText('No GPU')).toBeInTheDocument();
    fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('shows MIG slices as their own entries and says when nothing is collected', () => {
    ok();
    show();
    fireEvent.click(deviceButton('gh'));
    let dialog = screen.getByRole('dialog');
    expect(within(dialog).getByText('GPU 0 · MIG 1')).toBeInTheDocument();
    expect(within(dialog).getByText('GPU 0 · MIG 2')).toBeInTheDocument();
    expect(within(dialog).getByText('10.0.0.206')).toBeInTheDocument();
    expect(within(dialog).getByText('72 CPU cores · arm64 · GH200 x1')).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Close' }));
    fireEvent.click(deviceButton('early'));
    dialog = screen.getByRole('dialog');
    expect(within(dialog).getByText('No status is collected for this device.')).toBeInTheDocument();
    expect(within(dialog).getByText(/where it sits in the rack is not known yet/)).toBeInTheDocument();
  });

  it('asks again every 30 seconds through useApi and at once on Refresh', async () => {
    ok();
    show();
    expect(REFRESH_MS).toBe(30000);
    expect(useApi).toHaveBeenCalledWith('/api/rack/layout', 30000);
    expect(useApi).toHaveBeenCalledWith('/api/rack/status', 30000);
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Refresh' })); });
    expect(refetchLayout).toHaveBeenCalledTimes(1);
    expect(refetchStatus).toHaveBeenCalledTimes(1);
  });

  it('says the feature is off when it is not configured', () => {
    serve({ configured: false, layout: null, error: null }, { configured: false, status: null, error: null });
    show();
    expect(screen.getByText('The rack page is turned off')).toBeInTheDocument();
    expect(screen.getByText(/rack_layout_file/)).toBeInTheDocument();
    expect(document.querySelector('.rack-grid')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Refresh' })).toBeNull();
  });

  it('says why when the layout file cannot be used', () => {
    serve({ configured: true, layout: null, error: { code: 'invalid', detail: 'racks[0].units: 99 given; 1–60 allowed' } }, { configured: true, status: null, error: null });
    show();
    const alert = screen.getByRole('alert');
    expect(within(alert).getByText('The layout file is not in the expected form.')).toBeInTheDocument();
    expect(within(alert).getByText('racks[0].units: 99 given; 1–60 allowed')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Refresh' })).toBeInTheDocument();
  });

  it('still draws the racks when Prometheus cannot be reached, every node unknown', () => {
    const unknown = Object.fromEntries(Object.keys(status.nodes).map((n) => [n, node({ level: 'unknown', ready: null, cpu_pct: null, mem_pct: null, disk_pct: null, temp_c: null })]));
    const devices = Object.fromEntries(Object.entries(status.devices).map(([id, d]) => [id, { ...d, level: d.nodes.length ? 'unknown' as const : 'none' as const }]));
    serve({ configured: true, layout, error: null }, { configured: true, error: null, status: {
      ...status, source: 'unreachable', message: 'cannot connect', nodes: unknown, devices,
      summary: { nodes_total: 4, nodes_ready: 0, gpus: 0, warnings: 0, down: 0, max_temp_c: null },
    } });
    show();
    expect(screen.getByText(/Prometheus cannot be reached/)).toBeInTheDocument();
    expect(screen.getByText('cannot connect')).toBeInTheDocument();
    expect(deviceButton('gh').getAttribute('data-level')).toBe('unknown');
    expect(deviceButton('sw').getAttribute('data-level')).toBe('none');
    expect(document.querySelector('[data-tile="nodes"] .rack-tile-value')?.textContent).toBe('–/4');
    expect(document.querySelectorAll('.rack-cabinet')).toHaveLength(2);
  });

  it('keeps the drawing when the status request itself fails, and ignores a status for another layout version', () => {
    serve({ configured: true, layout, error: null }, null, { status: 'HTTP 502' });
    const first = show();
    expect(screen.getByText('No answer from the server: HTTP 502')).toBeInTheDocument();
    expect(deviceButton('gh').getAttribute('data-level')).toBe('unknown');
    expect(deviceButton('sw').getAttribute('data-level')).toBe('none');
    first.unmount();
    serve({ configured: true, layout, error: null }, { configured: true, error: null, status: { ...status, layout_version: 'older' } });
    show();
    expect(deviceButton('t4').getAttribute('data-level')).toBe('unknown');
    expect(document.querySelector('.rack-summary')).toBeNull();
  });
});
