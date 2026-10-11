import { useEffect, useState } from 'react';

/** A server-enforced limit is still authoritative after navigation or refresh. */
export function useAuthCooldown() {
  const [deadline, setDeadline] = useState(0);
  const [seconds, setSeconds] = useState(0);
  useEffect(() => {
    if (!deadline) return;
    const tick = () => {
      const remaining = Math.max(0, Math.ceil((deadline - Date.now()) / 1000));
      setSeconds(remaining);
      if (!remaining) setDeadline(0);
    };
    tick(); const timer = window.setInterval(tick, 250);
    return () => window.clearInterval(timer);
  }, [deadline]);
  const accept = (response: Response) => {
    if (response.status !== 429) return false;
    const raw = response.headers?.get('Retry-After') || '';
    const parsed = /^\d{1,6}$/.test(raw) ? Number(raw) : 60;
    const wait = Math.max(1, Math.min(300, parsed));
    setSeconds(wait); setDeadline(Date.now() + wait * 1000);
    return true;
  };
  return { seconds, accept };
}
