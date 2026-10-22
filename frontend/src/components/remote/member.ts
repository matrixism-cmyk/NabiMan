import { createContext, useContext } from 'react';

/**
 * Whether the remote workspace is open to a remote-only member (role
 * `remote`): their own servers only — no shell or files of this machine, no
 * one-off targets, no sharing, no SSH key of NabiMan's, no settings that are
 * the whole server's. The server refuses all of that anyway; this keeps it
 * from being offered.
 */
export const MemberContext = createContext(false);

export const useMember = () => useContext(MemberContext);

/**
 * What an administrator lets a member do beyond working on their servers
 * (the account's `remote_permissions`). Limits of 0 mean no limit.
 */
export interface MemberGrants {
  share_sessions: boolean;
  share_files: boolean;
  share_max_days: number;
  share_require_password: boolean;
  share_view_only: boolean;
  dictation: boolean;
  register_servers: boolean;
  assigned_file_write: boolean;
  copy_between_servers: boolean;
  keep_sessions: boolean;
  max_sessions: number;
  max_idle_minutes: number;
  max_servers: number;
  max_upload_mb: number;
}

/** What a new member may do: what members always could, and no sharing. */
export const DEFAULT_GRANTS: MemberGrants = {
  share_sessions: false, share_files: false, share_max_days: 0, share_require_password: false, share_view_only: false,
  dictation: true, register_servers: true, assigned_file_write: true, copy_between_servers: true, keep_sessions: true,
  max_sessions: 0, max_idle_minutes: 0, max_servers: 0, max_upload_mb: 0,
};

/** `null` for an administrator, who may do all of it. */
export const GrantsContext = createContext<MemberGrants | null>(null);

export const useGrants = () => useContext(GrantsContext);

/** The share links this account may make. */
export interface Sharing { sessions: boolean; files: boolean }

export const useSharing = (): Sharing => {
  const grants = useGrants();
  return { sessions: !grants || grants.share_sessions, files: !grants || grants.share_files };
};

/** For a member, a server an administrator assigned them: theirs to work on, not to edit. */
export const assignedTo = (member: boolean, server?: { owner_id?: string }) => member && Boolean(server) && !server?.owner_id;
