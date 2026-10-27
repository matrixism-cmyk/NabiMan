import { useEffect } from 'react';
import * as serialBridge from './serialBridge';
import * as serialDevices from './serialDevices';

type Translate = Parameters<typeof serialBridge.setTranslator>[0];
interface LiveSession { id: string; alive: boolean; label: string }

/**
 * Serial devices for an admin's workspace: the remembered global board is
 * re-bridged as soon as the workspace opens (not only on the Serial tab), and
 * session devices follow their sessions — those still alive come back with no
 * picker, ended ones are forgotten.
 */
export function useSerialWorkspace(enabled: boolean, t: Translate, sessions: LiveSession[] | null | undefined) {
  useEffect(() => {
    if (!enabled) return;
    serialBridge.setTranslator(t);
    void serialBridge.resume();
  }, [enabled, t]);
  useEffect(() => {
    if (!enabled || !sessions) return;
    serialDevices.pruneSessions(new Set(sessions.filter(s => s.alive).map(s => s.id)));
    for (const s of sessions) if (s.alive && s.label === 'local') serialDevices.resumeSession(s.id);
  }, [enabled, sessions]);
}
