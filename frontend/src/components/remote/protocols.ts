import { RemoteServer } from '../../types';

export type TerminalProtocol = 'ssh' | 'telnet' | 'none';
export type FileProtocol = 'sftp' | 'ftps' | 'ftp' | 'none';

/** Records saved before protocols could be chosen are SSH with SFTP. */
export const terminalOf = (s: RemoteServer): TerminalProtocol => s.terminal || 'ssh';
export const filesOf = (s: RemoteServer): FileProtocol => s.files || 'sftp';

export const hasTerminal = (s: RemoteServer) => terminalOf(s) !== 'none';
export const hasFiles = (s: RemoteServer) => filesOf(s) !== 'none';
/** Some part runs over SSH, so its host key is approved and keys can log in. */
export const usesSsh = (s: RemoteServer) => terminalOf(s) === 'ssh' || filesOf(s) === 'sftp';
/** One-off commands and the machine's vital signs come over an SSH shell. */
export const hasShell = (s: RemoteServer) => terminalOf(s) === 'ssh';

/** Protocols that send passwords and data unencrypted. */
export const PLAINTEXT = new Set<string>(['telnet', 'ftp']);

const NAMES: Record<string, string> = { ssh: 'SSH', telnet: 'Telnet', sftp: 'SFTP', ftps: 'FTPS', ftp: 'FTP' };
export const protocolName = (p: string) => NAMES[p] || p.toUpperCase();

/** What a server is reached by, terminal first: [] never, since a record
 *  needs at least one. */
export function protocolsOf(s: RemoteServer): string[] {
  return [terminalOf(s), filesOf(s)].filter(p => p !== 'none');
}

/** `user@host:port` for the way it is mostly used: its terminal, or its
 *  files when there is no terminal. */
export function addressOf(s: RemoteServer): string {
  const port = !hasTerminal(s) && (filesOf(s) === 'ftp' || filesOf(s) === 'ftps') ? s.ftp_port || 21 : s.port;
  return `${s.user}@${s.host}:${port}`;
}

/** The label the backend gives a terminal session on this server. */
export function sessionLabel(s: RemoteServer): string {
  return `${terminalOf(s) === 'telnet' ? 'telnet' : 'ssh'}:${s.user}@${s.host}:${s.port}`;
}

/** Filter choices in the server list; `ftp` covers FTPS too. */
export const PROTOCOL_FILTERS = ['ssh', 'telnet', 'sftp', 'ftp'] as const;
export type ProtocolFilter = typeof PROTOCOL_FILTERS[number] | '';

export function matchesProtocol(s: RemoteServer, filter: ProtocolFilter): boolean {
  if (!filter) return true;
  if (filter === 'ftp') return filesOf(s) === 'ftp' || filesOf(s) === 'ftps';
  return protocolsOf(s).includes(filter);
}
