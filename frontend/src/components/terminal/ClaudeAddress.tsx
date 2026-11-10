import React, { useEffect, useState } from 'react';
import { apiRequest } from '../../shared/api/useApi';
import { useT } from '../../shared/i18n';
import './claude-address.css';

/** A Claude Code session running in a terminal session. */
export interface ClaudePeer {
  name: string;
  ref: string;
  /** What another Claude session's SendMessage `to` takes: `name [ref]`. */
  address: string;
  cwd: string;
  /** Working on a turn now. */
  busy: boolean;
  /** When it last became busy or idle (seconds since 1970), if known. */
  since?: number;
  /** The model that last answered in it (`claude-opus-5-5`…). */
  model?: string;
  /** Its Claude Code version. */
  version?: string;
  /** The saved server it runs on, when not this machine; `address` is then
   *  `bridge:<Remote Control session>`, or just the name without Remote Control. */
  server?: string;
}

interface ClaudeInfo { claude: ClaudePeer[]; busy: boolean }

/** How often an open terminal looks again: Claude may start or finish meanwhile. */
const EVERY_MS = 10000;

/** The Claude sessions in one terminal session, kept up to date while it is open. */
export function useClaudePeers(sessionId: string | undefined): ClaudePeer[] {
  const [peers, setPeers] = useState<ClaudePeer[]>([]);
  useEffect(() => {
    setPeers([]);
    if (!sessionId) return undefined;
    let live = true;
    const look = () => {
      if (document.visibilityState === 'hidden') return;
      apiRequest<ClaudeInfo>(`/api/terminal/sessions/${encodeURIComponent(sessionId)}/claude`)
        .then((res) => { if (live && res.success) setPeers(res.data?.claude || []); })
        .catch(() => { /* keep what was shown */ });
    };
    look();
    const timer = window.setInterval(look, EVERY_MS);
    document.addEventListener('visibilitychange', look);
    return () => { live = false; window.clearInterval(timer); document.removeEventListener('visibilitychange', look); };
  }, [sessionId]);
  return peers;
}

/** Copies text, also where the async clipboard is unavailable (plain http). */
export async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch { /* fall back below */ }
  const area = document.createElement('textarea');
  area.value = text;
  area.setAttribute('readonly', '');
  area.style.position = 'fixed';
  area.style.opacity = '0';
  document.body.appendChild(area);
  area.select();
  try {
    return document.execCommand('copy');
  } catch {
    return false;
  } finally {
    area.remove();
  }
}

interface Props {
  peers: ClaudePeer[];
  /** `bar` for a window's title bar, `card` for the session list. */
  variant?: 'head' | 'bar' | 'card';
}

/**
 * The address each Claude session in this terminal goes by — `artisland-bd
 * [946528]`, as ListAgents shows it — with its folder on hover; a click
 * copies it, ready for another Claude session's SendMessage.
 */
export default function ClaudeAddress({ peers, variant = 'head' }: Props) {
  const { t } = useT();
  const [copied, setCopied] = useState('');
  useEffect(() => {
    if (!copied) return undefined;
    const timer = window.setTimeout(() => setCopied(''), 1500);
    return () => window.clearTimeout(timer);
  }, [copied]);
  if (peers.length === 0) return null;
  return (
    <span className={`claude-addr claude-addr-${variant}`}>
      {peers.map((p) => {
        const done = copied === p.address;
        const state = t(p.busy ? 'claudePeer.busy' : 'claudePeer.idle');
        const reachable = !p.server || p.address.startsWith('bridge:');
        const hint = p.server
          ? t(reachable ? 'claudePeer.remoteHint' : 'claudePeer.unreachableHint', { address: p.address, cwd: p.cwd, state, server: p.server })
          : t('claudePeer.hint', { address: p.address, cwd: p.cwd, state });
        return (
          <button key={p.address} type="button" className={`claude-addr-chip ${p.busy ? 'is-busy' : ''} ${p.server ? 'is-remote' : ''} ${reachable ? '' : 'is-unreachable'}`}
            title={hint}
            aria-label={t('claudePeer.copyLabel', { address: p.address })}
            onPointerDown={(e) => e.stopPropagation()}
            onClick={async (e) => {
              e.stopPropagation();
              if (await copyText(p.address)) setCopied(p.address);
            }}>
            <span className="claude-addr-dot" aria-hidden="true" />
            <span className="claude-addr-name">{p.name}</span>
            {p.server ? <span className="claude-addr-ref">@{p.server}</span> : <span className="claude-addr-ref">[{p.ref}]</span>}
            {variant !== 'bar' && <span className="claude-addr-cwd">{p.cwd}</span>}
            <span className="claude-addr-copy">{done ? t('claudePeer.copied') : '⧉'}</span>
          </button>
        );
      })}
    </span>
  );
}
