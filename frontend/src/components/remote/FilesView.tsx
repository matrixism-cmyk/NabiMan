import React, { useMemo } from 'react';
import { RemoteServer } from './remoteTypes';
import SftpPanel from './sftp/SftpPanel';
import { accountSource } from './sftp/sftpApi';
import './sftp/sftp.css';

/** A server reached only for its files: the file panel is the whole view. */
export default function FilesView({ server }: { server: RemoteServer }) {
  const source = useMemo(() => accountSource(server.id), [server.id]);
  return (
    <div className="rw-view rw-view-files">
      <SftpPanel key={server.id} source={source} serverName={server.name} />
    </div>
  );
}
