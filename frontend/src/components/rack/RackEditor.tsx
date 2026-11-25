import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useT } from '../../i18n';
import { apiRequest } from '../../shared/api/useApi';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import { setLeaveGuard } from '../../shared/ui/leaveGuard';
import type { RackDevice, RackLayout, RackNode, RackStatus } from './rackTypes';
import { findDevice } from './rackEditGeometry';
import { slotLabel } from './rackGeometry';
import RackCabinet from './RackCabinet';
import RackEditBar from './RackEditBar';
import { MoveResult, useRackEdit } from './useRackEdit';
import { useRackDrag } from './useRackDrag';
import './rackEdit.css';

interface Props {
  /** The layout as it was when editing began; it is not swapped for a newer one while editing. */
  base: RackLayout;
  /** The version the server has now (the page keeps asking): another than `base`'s means somebody else saved. */
  liveVersion: string | null;
  nodes: Record<string, RackNode>;
  status: RackStatus | null;
  /** Saved: the layout the server answered with. */
  onSaved: (layout: RackLayout) => void;
  /** Editing ended without a save. */
  onClose: () => void;
  /** Give up the moves and show what the server has now. */
  onReload: () => void;
}

/** What POST /api/rack/layout/moves answers, saved or not. */
interface SaveAnswer { success: boolean; code?: string; message?: string; data?: { layout?: RackLayout; current_version?: string } | null }

const ARROWS = ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'] as const;
type Arrow = typeof ARROWS[number];

/**
 * The racks while an administrator moves devices: drag a device (or use the
 * arrow keys) to a free place, undo, and save all moves at once as a new
 * version of the layout file. Nothing is sent before Save.
 */
export default function RackEditor({ base, liveVersion, nodes, status, onSaved, onClose, onReload }: Props) {
  const { t } = useT();
  const root = useRef<HTMLDivElement>(null);
  const edit = useRackEdit(base);
  const [said, setSaid] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<{ code: string; detail: string } | null>(null);
  const [refused, setRefused] = useState<string | null>(null);
  const [refocus, setRefocus] = useState<{ id: string } | null>(null);
  const count = edit.moves.length;
  const conflict = refused ?? (liveVersion !== null && liveVersion !== base.version ? liveVersion : null);

  const tell = useCallback((device: RackDevice, result: MoveResult) => {
    if (!result.ok) { setSaid(t(`rack.edit.said.${result.reason}`, { label: device.label, other: result.other ?? '' })); return; }
    const rack = base.racks.find((r) => r.id === result.rack);
    setSaid(t('rack.edit.said.moved', { label: device.label, where: `${rack?.name ?? result.rack} ${slotLabel(result.u, device.h)}` }));
  }, [base, t]);

  const onDrop = useCallback((id: string, rack: string, u: number) => {
    const found = findDevice(edit.working, id);
    if (!found) return;
    setError(null);
    tell(found.device, edit.move(id, rack, u));
  }, [edit, tell]);
  const { dragging, onPointerDown } = useRackDrag(root, edit.working, onDrop);

  const onKeyDown = useCallback((e: React.KeyboardEvent<HTMLElement>, device: RackDevice) => {
    if (!ARROWS.includes(e.key as Arrow) || e.altKey || e.ctrlKey || e.metaKey) return;
    e.preventDefault();
    setError(null);
    tell(device, edit.nudge(device, e.key as Arrow));
    // In another rack it is a new element: focus follows the device.
    setRefocus({ id: device.id });
  }, [edit, tell]);
  useEffect(() => {
    if (!refocus) return;
    const face = Array.from(root.current?.querySelectorAll<HTMLElement>('[data-device]') ?? []).find((el) => el.dataset.device === refocus.id);
    if (face && document.activeElement !== face) face.focus();
  }, [refocus, edit.working]);

  // Unsaved moves: ask before the tab closes or the app shows another page.
  useEffect(() => {
    if (count === 0) return undefined;
    const warn = (e: BeforeUnloadEvent) => { e.preventDefault(); e.returnValue = ''; };
    window.addEventListener('beforeunload', warn);
    const release = setLeaveGuard(() => confirmDialog({ message: t('rack.edit.leaveConfirm'), confirmLabel: t('rack.edit.discard'), danger: true }));
    return () => { window.removeEventListener('beforeunload', warn); release(); };
  }, [count, t]);

  const cancel = async () => {
    if (count > 0 && !(await confirmDialog({ message: t('rack.edit.cancelConfirm', { count }), confirmLabel: t('rack.edit.discard'), danger: true }))) return;
    onClose();
  };
  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      const answer = await apiRequest<unknown>('/api/rack/layout/moves', { method: 'POST', body: JSON.stringify({ base_version: base.version, moves: edit.moves }) }) as unknown as SaveAnswer;
      if (answer.success && answer.data?.layout) onSaved(answer.data.layout);
      else if (answer.code === 'version_conflict') setRefused(answer.data?.current_version || '?');
      else setError({ code: answer.code ?? 'other', detail: answer.message ?? '' });
    } catch (e) {
      setError({ code: 'other', detail: e instanceof Error ? e.message : '' });
    } finally { setSaving(false); }
  };

  const moved = useMemo(() => new Set(edit.moves.map((m) => m.id)), [edit.moves]);
  const origins = useMemo(() => edit.moves.flatMap((m) => {
    const was = findDevice(base, m.id);
    return was && was.device.u !== null ? [{ rack: was.rack.id, id: m.id, label: was.device.label, u: was.device.u, h: was.device.h }] : [];
  }), [base, edit.moves]);
  const noOpen = useCallback(() => {}, []);

  return (
    <div ref={root} className={`rack-editor${dragging ? ' is-dragging' : ''}`}>
      <RackEditBar count={count} canUndo={edit.steps.length > 0} saving={saving} error={error} conflict={conflict} said={said}
        onSave={save} onUndo={() => { edit.undo(); setSaid(t('rack.edit.said.undone')); }} onCancel={cancel} onReload={onReload} />
      <div className="rack-grid" data-racks={edit.working.racks.length}>
        {edit.working.racks.map((rack) => (
          <RackCabinet key={rack.id} rack={rack} nodes={nodes} status={status} onOpen={noOpen}
            edit={{ moved, origins: origins.filter((o) => o.rack === rack.id), dragging, onPointerDown, onKeyDown }} />
        ))}
      </div>
    </div>
  );
}
