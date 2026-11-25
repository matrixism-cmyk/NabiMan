import { createContext } from 'react';
import type { SftpSource } from './sftp/sftpApi';

/** What the file panel beside a terminal offers the terminal itself. */
export interface TerminalFiles {
  /** Where this terminal's files come from. */
  source: SftpSource;
  /** Opens the panel at a folder. */
  showFolder: (path: string) => void;
}

/**
 * Set by TerminalWithFiles for the terminal inside it; null where a terminal
 * has no files to show (an unsaved target, a share link without file access).
 */
export const TerminalFilesContext = createContext<TerminalFiles | null>(null);
