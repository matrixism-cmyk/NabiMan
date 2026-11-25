import { useCallback, useMemo, useState } from 'react';
import type { RackDevice, RackLayout } from './rackTypes';
import { PlaceCheck, Step, applySteps, checkPlace, findDevice, neighbour, netMoves } from './rackEditGeometry';

/** What a try at moving a device came to. */
export type MoveResult = { ok: true; rack: string; u: number } | { ok: false; reason: 'outside' | 'overlap' | 'edge' | 'unplaced'; other?: string };

export interface RackEdit {
  /** The layout the editor started from; not replaced while editing. */
  base: RackLayout;
  /** `base` with the moves made so far. */
  working: RackLayout;
  steps: Step[];
  /** What a save would send: devices that are somewhere else than in `base`. */
  moves: { id: string; rack: string; u: number }[];
  /** Whether the device may go there as things stand now. */
  check: (id: string, rack: string, u: number) => PlaceCheck;
  move: (id: string, rack: string, u: number, keys?: boolean) => MoveResult;
  /** One slot up or down, or to the rack beside it at the same slot. */
  nudge: (device: RackDevice, key: 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight') => MoveResult;
  undo: () => void;
}

/** The moves of one editing session over `base`, each checked as it is made. */
export function useRackEdit(base: RackLayout): RackEdit {
  const [steps, setSteps] = useState<Step[]>([]);
  const working = useMemo(() => applySteps(base, steps), [base, steps]);
  const moves = useMemo(() => netMoves(base, steps), [base, steps]);

  const check = useCallback((id: string, rackId: string, u: number): PlaceCheck => {
    const rack = working.racks.find((r) => r.id === rackId);
    const found = findDevice(working, id);
    if (!rack || !found) return { ok: false, reason: 'outside' };
    return checkPlace(rack, id, u, found.device.h);
  }, [working]);

  const move = useCallback((id: string, rack: string, u: number, keys = false): MoveResult => {
    const found = findDevice(working, id);
    const checked = check(id, rack, u);
    if (!found || !checked.ok) return checked.ok ? { ok: false, reason: 'outside' } : checked;
    const from = { rack: found.rack.id, u: found.device.u };
    if (from.rack === rack && from.u === u) return { ok: true, rack, u };
    setSteps((made) => {
      const last = made[made.length - 1];
      // A run of arrow keys on one device is one move to undo.
      if (keys && last?.keys && last.id === id) {
        const joined = { ...last, to: { rack, u } };
        return joined.from.rack === rack && joined.from.u === u ? made.slice(0, -1) : [...made.slice(0, -1), joined];
      }
      return [...made, { id, from, to: { rack, u }, keys }];
    });
    return { ok: true, rack, u };
  }, [working, check]);

  const nudge = useCallback((device: RackDevice, key: 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight'): MoveResult => {
    const found = findDevice(working, device.id);
    if (!found || found.device.u === null) return { ok: false, reason: 'unplaced' };
    if (key === 'ArrowUp' || key === 'ArrowDown') return move(device.id, found.rack.id, found.device.u + (key === 'ArrowUp' ? 1 : -1), true);
    const beside = neighbour(working, found.rack.id, key === 'ArrowLeft' ? -1 : 1);
    return beside ? move(device.id, beside.id, found.device.u, true) : { ok: false, reason: 'edge' };
  }, [working, move]);

  const undo = useCallback(() => setSteps((made) => made.slice(0, -1)), []);

  return { base, working, steps, moves, check, move, nudge, undo };
}
