import React from 'react';
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { apiRequest, useApi } from '../../shared/api/useApi';
import { guardedLeave } from '../../shared/ui/leaveGuard';
import RackPanel from './RackPanel';
import type { RackDevice, RackLayout, RackStatus } from './rackTypes';

jest.mock('../../shared/api/useApi', () => ({ useApi: jest.fn(), apiRequest: jest.fn() }));

const device = (over: Partial<RackDevice> & { id: string }): RackDevice => ({
  label: over.id, type: 'server', u: null, h: 1, u_confirmed: false, node: null, node_candidates: [], ip: null, bmc: null,
  gpu: null, cpu: null, arch: null, desc: null, order_hint: null, check: 'ping', external: false, ...over,
});

const layout: RackLayout = {
  version: 'v-test', title: 'Test server room', sources: [], notes: null, power: [], overlaps: [], nodes: [],
  racks: [
    { id: 'R1', name: 'RACK #1', role: null, units: 42, external: false, devices: [
      device({ id: 'sw', label: 'Core switch', type: 'switch', u: 38, u_confirmed: true, desc: '' }),
      device({ id: 'lenovo', label: 'SR650 #1', u: 27, h: 2, desc: 'which node is not known' }),
    ] },
    { id: 'R2', name: 'RACK #2', role: null, units: 42, external: false, devices: [
      device({ id: 't4', label: 'T4 box', u: 36, h: 2 }),
      device({ id: 'late', label: 'Placed later', type: 'storage', h: 2 }),
    ] },
  ],
};
const status: RackStatus = {
  collected_at: '2026-11-25T12:00:00Z', source: 'ok', message: null, layout_version: 'v-test', nodes: {}, devices: {},
  summary: { nodes_total: 0, nodes_ready: 0, gpus: 0, warnings: 0, down: 0, max_temp_c: null, probed: 0, reachable: 0, noreply: 0, external_noreply: 0, over_limit: 0 },
  thresholds: { temp_c: 80, memory_pct: 90, disk_pct: 90 },
};

const refetch = jest.fn(async () => {});
/** What the server has: the layout it serves and who is asking. */
let served: { layout: RackLayout; role: string };

/** Each rack is drawn 400 px wide and 840 px tall (20 px a slot) from y=100; RACK #2 starts at x=500. */
const LEFT: Record<string, number> = { R1: 0, R2: 500 };
const box = (left: number, top: number, width: number, height: number) => ({ left, top, width, height, right: left + width, bottom: top + height, x: left, y: top, toJSON: () => ({}) }) as DOMRect;
const yOf = (u: number) => 100 + (42 - u) * 20;

class TestPointerEvent extends MouseEvent {
  pointerId: number;
  isPrimary: boolean;
  constructor(type: string, init: MouseEventInit & { pointerId?: number }) { super(type, init); this.pointerId = init.pointerId ?? 1; this.isPrimary = true; }
}

beforeEach(() => {
  served = { layout, role: 'admin' };
  (useApi as jest.Mock).mockImplementation((endpoint: string) => {
    const data = endpoint === '/api/rack/layout' ? { configured: true, layout: served.layout, error: null }
      : endpoint === '/api/rack/status' ? { configured: true, status, error: null } : { role: served.role };
    return { data, loading: false, error: null, refetch };
  });
  refetch.mockImplementation(async () => {});
  // jsdom has neither layout nor pointer events.
  (window as unknown as { PointerEvent: unknown }).PointerEvent = TestPointerEvent;
  jest.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function measure(this: HTMLElement) {
    const slots = this.closest<HTMLElement>('[data-rack-slots]');
    if (!slots) return box(0, 0, 0, 0);
    const left = LEFT[slots.dataset.rackSlots as string];
    if (this === slots) return box(left, 100, 400, 840);
    return box(left, 100 + Number(this.style.getPropertyValue('--top')) * 20, 400, Number(this.style.getPropertyValue('--h')) * 20);
  });
});
afterEach(() => { jest.restoreAllMocks(); });

function show() {
  localStorage.setItem('nabiman_lang', 'en');
  return render(<I18nProvider><RackPanel /></I18nProvider>);
}
const face = (id: string) => document.querySelector(`[data-device="${id}"]`) as HTMLElement;
const rackOf = (id: string) => face(id).closest('[data-rack]')?.getAttribute('data-rack');
const pointer = (type: string, target: EventTarget, x: number, y: number) => act(() => {
  fireEvent(target as Element, new TestPointerEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 0 }));
});
/** Pick `id` up 10 px under its top edge and carry it to slot `u` of the rack at `x`. */
function carry(id: string, x: number, u: number) {
  const from = face(id).getBoundingClientRect();
  pointer('pointerdown', face(id), from.left + 50, from.top + 10);
  pointer('pointermove', window, x, yOf(u) + 10);
}
const drag = (id: string, x: number, u: number) => { carry(id, x, u); pointer('pointerup', window, x, yOf(u) + 10); };
const startEditing = () => fireEvent.click(screen.getByRole('button', { name: /Edit positions/ }));
const saveButton = () => document.querySelector('[data-save]') as HTMLButtonElement;

describe('editing positions on the rack page', () => {
  it('shows the edit button to administrators only, and changes nothing until it is used', () => {
    served.role = 'operator';
    const first = show();
    expect(screen.queryByRole('button', { name: /Edit positions/ })).toBeNull();
    expect(document.querySelector('[data-editing]')).toBeNull();
    first.unmount();
    served.role = 'admin';
    show();
    expect(screen.getByRole('button', { name: /Edit positions/ })).toBeTruthy();
    expect(document.querySelector('[data-editing], .is-editable, [data-origin], [data-drop]')).toBeNull();
    // Outside edit mode a press and a pull do nothing, and a click opens the details.
    drag('sw', 100, 30);
    expect(face('sw').style.getPropertyValue('--top')).toBe('4');
    fireEvent.click(face('sw'));
    expect(screen.getByRole('dialog')).toBeTruthy();
  });

  it('never shows a row for an empty description', () => {
    show();
    fireEvent.click(face('sw'));
    expect(within(screen.getByRole('dialog')).queryByText('Notes')).toBeNull();
    fireEvent.click(within(screen.getByRole('dialog')).getAllByRole('button', { name: 'Close' })[0]);
    fireEvent.click(face('lenovo'));
    expect(within(screen.getByRole('dialog')).getByText('Notes')).toBeTruthy();
  });

  it('enters edit mode: a banner, nothing to save yet, and a click opens no details', () => {
    show();
    startEditing();
    expect(screen.getByText(/Editing — nothing changes until you save/)).toBeTruthy();
    expect(saveButton().disabled).toBe(true);
    expect(saveButton().textContent).toBe('Save');
    expect((screen.getByRole('button', { name: 'Undo' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.queryByRole('button', { name: /Edit positions/ })).toBeNull();
    fireEvent.click(face('sw'));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(face('sw').className).toContain('is-editable');
  });

  it('drags a device to a free slot: the landing place is shown while dragging and the move is marked after', () => {
    show();
    startEditing();
    carry('sw', 100, 30);
    const drop = document.querySelector('[data-drop]') as HTMLElement;
    expect(drop.dataset.drop).toBe('valid');
    expect(drop.textContent).toContain('U30');
    expect(drop.style.getPropertyValue('--top')).toBe('12');
    expect(face('sw').className).toContain('is-held');
    pointer('pointerup', window, 100, yOf(30) + 10);
    expect(document.querySelector('[data-drop]')).toBeNull();
    expect(face('sw').style.getPropertyValue('--top')).toBe('12');
    expect(within(face('sw')).getByText('Moved')).toBeTruthy();
    expect((document.querySelector('[data-origin="sw"]') as HTMLElement).style.getPropertyValue('--top')).toBe('4');
    expect(saveButton().disabled).toBe(false);
    expect(saveButton().textContent).toBe('Save (1 moved)');
    expect(document.querySelector('[data-said]')?.textContent).toBe('Core switch is now at RACK #1 U30.');
    // A press without a pull is not a move.
    pointer('pointerdown', face('lenovo'), 50, yOf(27) + 10);
    pointer('pointerup', window, 50, yOf(27) + 10);
    expect(saveButton().textContent).toBe('Save (1 moved)');
  });

  it('refuses a place that overlaps another device or lies outside the rack, and says why', () => {
    show();
    startEditing();
    carry('sw', 100, 26);
    let drop = document.querySelector('[data-drop]') as HTMLElement;
    expect(drop.dataset.drop).toBe('invalid');
    expect(drop.textContent).toContain('Overlaps SR650 #1');
    pointer('pointerup', window, 100, yOf(26) + 10);
    expect(face('sw').style.getPropertyValue('--top')).toBe('4');
    expect(saveButton().disabled).toBe(true);
    // The 2U server pulled past the bottom: shown at the lowest place, refused.
    carry('lenovo', 100, 0);
    drop = document.querySelector('[data-drop]') as HTMLElement;
    expect([drop.dataset.drop, drop.style.getPropertyValue('--top')]).toEqual(['invalid', '40']);
    expect(drop.textContent).toContain('Outside the rack');
    pointer('pointerup', window, 100, yOf(0) + 10);
    // Let go between the racks: back where it was.
    drag('lenovo', 450, 10);
    expect(document.querySelector('[data-drop]')).toBeNull();
    expect(face('lenovo').style.getPropertyValue('--top')).toBe('15');
    // Escape gives a drag up.
    carry('sw', 100, 30);
    act(() => { fireEvent.keyDown(window, { key: 'Escape' }); });
    expect(document.querySelector('[data-drop]')).toBeNull();
    pointer('pointerup', window, 100, yOf(30) + 10);
    expect(saveButton().disabled).toBe(true);
  });

  it('moves a device to another rack, places one that had no slot, undoes, and sends only what ends up moved', async () => {
    (apiRequest as jest.Mock).mockResolvedValue({ success: false, code: 'write_failed', message: 'The layout was not saved: Permission denied', data: { code: 'write_failed' } });
    show();
    startEditing();
    drag('sw', 600, 10);
    expect(rackOf('sw')).toBe('R2');
    expect(document.querySelector('[data-rack="R1"] [data-origin="sw"]')).toBeTruthy();
    drag('late', 100, 20); // held by its middle: a 2U device centred on the pointer
    expect([rackOf('late'), face('late').className.includes('is-listed')]).toEqual(['R1', false]);
    drag('lenovo', 100, 33);
    drag('lenovo', 100, 27); // back where it was: not a move to save
    expect(saveButton().textContent).toBe('Save (2 moved)');
    drag('t4', 600, 30);
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    expect(face('t4').style.getPropertyValue('--top')).toBe('6');
    expect(saveButton().textContent).toBe('Save (2 moved)');
    await act(async () => { fireEvent.click(saveButton()); });
    expect(apiRequest).toHaveBeenCalledTimes(1);
    const [url, options] = (apiRequest as jest.Mock).mock.calls[0];
    expect([url, options.method]).toEqual(['/api/rack/layout/moves', 'POST']);
    expect(JSON.parse(options.body)).toEqual({ base_version: 'v-test', moves: [{ id: 'sw', rack: 'R2', u: 10 }, { id: 'late', rack: 'R1', u: 20 }] });
    // A failed save says so, keeps the moves and stays in edit mode.
    const failed = document.querySelector('[data-save-error]') as HTMLElement;
    expect(failed.textContent).toContain('could not be written');
    expect(failed.textContent).toContain('Permission denied');
    expect(rackOf('sw')).toBe('R2');
    for (let i = 0; i < 6; i += 1) fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    expect([rackOf('sw'), rackOf('late'), saveButton().disabled]).toEqual(['R1', 'R2', true]);
  });

  it('asks before throwing unsaved moves away, on Cancel and on leaving', async () => {
    const ask = jest.spyOn(window, 'confirm').mockReturnValue(false);
    show();
    startEditing();
    drag('sw', 100, 30);
    const leaving = new Event('beforeunload', { cancelable: true });
    window.dispatchEvent(leaving);
    expect(leaving.defaultPrevented).toBe(true);
    const go = jest.fn();
    await act(async () => { guardedLeave(go); });
    expect([ask.mock.calls.length, go.mock.calls.length]).toEqual([1, 0]);
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Cancel' })); });
    expect(ask).toHaveBeenCalledTimes(2);
    expect(ask.mock.calls[1][0]).toContain('1 moved device');
    expect(document.querySelector('[data-editing]')).toBeTruthy();
    ask.mockReturnValue(true);
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Cancel' })); });
    expect(document.querySelector('[data-editing]')).toBeNull();
    expect(face('sw').style.getPropertyValue('--top')).toBe('4');
    // Nothing unsaved: no question, here or on leaving.
    startEditing();
    const calm = new Event('beforeunload', { cancelable: true });
    window.dispatchEvent(calm);
    guardedLeave(go);
    expect([calm.defaultPrevented, go.mock.calls.length]).toEqual([false, 1]);
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Cancel' })); });
    expect(ask).toHaveBeenCalledTimes(3);
    expect(document.querySelector('[data-editing]')).toBeNull();
  });

  it('after a save shows the new version and leaves edit mode', async () => {
    const moved: RackLayout = { ...layout, version: 'v-new', racks: [{ ...layout.racks[0], devices: [{ ...layout.racks[0].devices[0], u: 30 }, layout.racks[0].devices[1]] }, layout.racks[1]] };
    (apiRequest as jest.Mock).mockResolvedValue({ success: true, data: { configured: true, layout: moved, error: null, version: 'v-new', file: 'rack_layout_x.json' }, message: 'OK' });
    refetch.mockImplementation(async () => { served.layout = moved; });
    show();
    startEditing();
    drag('sw', 100, 30);
    await act(async () => { fireEvent.click(saveButton()); });
    expect(document.querySelector('[data-editing]')).toBeNull();
    expect(document.querySelector('[data-saved]')?.textContent).toBe('Saved. The new layout version is v-new.');
    expect(screen.getByText('Layout version v-new')).toBeTruthy();
    expect(face('sw').style.getPropertyValue('--top')).toBe('12');
    expect(document.querySelector('[data-origin], [data-moved]')).toBeNull();
    expect(refetch).toHaveBeenCalled();
    expect(screen.getByRole('button', { name: /Edit positions/ })).toBeTruthy();
  });

  it('on a conflict says who won, saves nothing more, and reloads only when asked', async () => {
    (apiRequest as jest.Mock).mockResolvedValue({ success: false, code: 'version_conflict', message: 'conflict', data: { code: 'version_conflict', current_version: 'v-other' } });
    show();
    startEditing();
    drag('sw', 100, 30);
    await act(async () => { fireEvent.click(saveButton()); });
    const conflict = document.querySelector('[data-conflict]') as HTMLElement;
    expect(conflict.getAttribute('role')).toBe('alert');
    expect(conflict.textContent).toContain('v-other');
    expect(conflict.textContent).toContain('discards the moves you made');
    expect(saveButton().disabled).toBe(true);
    expect(face('sw').style.getPropertyValue('--top')).toBe('12');
    const theirs: RackLayout = { ...layout, version: 'v-other' };
    refetch.mockImplementation(async () => { served.layout = theirs; });
    await act(async () => { fireEvent.click(within(conflict).getByRole('button', { name: 'Reload' })); });
    expect(document.querySelector('[data-editing]')).toBeNull();
    expect(screen.getByText('Layout version v-other')).toBeTruthy();
    expect(face('sw').style.getPropertyValue('--top')).toBe('4');
  });

  it('keeps the layout being edited when the server gets a newer one, and says so', () => {
    const view = show();
    startEditing();
    drag('sw', 100, 30);
    served.layout = { ...layout, version: 'v-other', racks: [{ ...layout.racks[0], devices: [layout.racks[0].devices[1]] }, layout.racks[1]] };
    view.rerender(<I18nProvider><RackPanel /></I18nProvider>);
    expect(face('sw').style.getPropertyValue('--top')).toBe('12');
    expect(document.querySelector('[data-conflict]')?.textContent).toContain('v-other');
    expect(saveButton().disabled).toBe(true);
  });

  it('moves the focused device with the arrow keys and says what happened', () => {
    show();
    startEditing();
    face('sw').focus();
    fireEvent.keyDown(face('sw'), { key: 'ArrowUp' });
    fireEvent.keyDown(face('sw'), { key: 'ArrowUp' });
    expect(face('sw').style.getPropertyValue('--top')).toBe('2');
    expect(document.querySelector('[data-said]')?.textContent).toBe('Core switch is now at RACK #1 U40.');
    expect(document.querySelector('[data-said]')?.getAttribute('aria-live')).toBe('polite');
    fireEvent.keyDown(face('sw'), { key: 'ArrowRight' });
    expect(rackOf('sw')).toBe('R2');
    expect(document.activeElement).toBe(face('sw'));
    fireEvent.keyDown(face('sw'), { key: 'ArrowRight' });
    expect(document.querySelector('[data-said]')?.textContent).toBe('Core switch: there is no rack on that side.');
    // Down to the T4 box (U35–36): stopped above it.
    for (let i = 0; i < 5; i += 1) fireEvent.keyDown(face('sw'), { key: 'ArrowDown' });
    expect(face('sw').style.getPropertyValue('--top')).toBe('5');
    expect(document.querySelector('[data-said]')?.textContent).toBe('Core switch: cannot move there, it would overlap T4 box.');
    expect(saveButton().textContent).toBe('Save (1 moved)');
    // The whole run of keys is one move to undo.
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    expect([rackOf('sw'), face('sw').style.getPropertyValue('--top'), saveButton().disabled]).toEqual(['R1', '4', true]);
    fireEvent.keyDown(face('late'), { key: 'ArrowUp' });
    expect(document.querySelector('[data-said]')?.textContent).toContain('has no slot yet');
    fireEvent.keyDown(face('lenovo'), { key: 'Enter' });
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
