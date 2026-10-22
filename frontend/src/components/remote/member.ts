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
 * The share links this account may make: an administrator always both, a
 * member what an administrator granted (세션 공유, 파일 공유).
 */
export interface Sharing { sessions: boolean; files: boolean }
export const SharingContext = createContext<Sharing>({ sessions: true, files: true });

export const useSharing = () => useContext(SharingContext);

/** For a member, a server an administrator assigned them: theirs to work on, not to edit. */
export const assignedTo = (member: boolean, server?: { owner_id?: string }) => member && Boolean(server) && !server?.owner_id;
