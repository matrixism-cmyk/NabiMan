import React from 'react';
import { RemoteServer } from '../../types';
import { useT } from '../../i18n';
import { PLAINTEXT, protocolName, protocolsOf } from './protocols';

/** "SSH · SFTP", or "SFTP 전용" when there is just one. */
export function describeProtocols(t: (key: string, vars?: Record<string, string | number>) => string, protocols: string[]): string {
  const names = protocols.filter(p => p !== 'none').map(protocolName);
  return names.length === 1 ? t('remote.proto.only', { name: names[0] }) : names.join(' · ');
}

/** Small tags naming how a server is reached; unencrypted ones stand out. */
export default function ProtocolBadges({ server }: { server: RemoteServer }) {
  const { t } = useT();
  return (
    <span className="proto-badges">
      {protocolsOf(server).map(p => (
        <span key={p} className={`proto-badge proto-${p} ${PLAINTEXT.has(p) ? 'is-plain' : ''}`}
          title={PLAINTEXT.has(p) ? t('remote.proto.plain') : undefined}>
          {protocolName(p)}
        </span>
      ))}
    </span>
  );
}
