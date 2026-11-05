import type { ILinkHandler, Terminal } from 'xterm';

/**
 * Links in a terminal: a URL printed as text (the web-links addon finds it)
 * and an OSC 8 hyperlink, whose text can be anything — Claude Code prints
 * them, often as a short label or a shortened URL. Both open in a new tab on
 * a click, with no way back to this page.
 *
 * Any output can carry a link, a file someone else wrote shown with cat
 * included, so only web addresses are opened, and the address itself is shown
 * on hover: the label of an OSC 8 link need not be where it goes.
 */

/** The address to open, or null for anything but http(s). */
export function safeLinkTarget(uri: string): string | null {
  try {
    const url = new URL(uri);
    return url.protocol === 'http:' || url.protocol === 'https:' ? url.href : null;
  } catch {
    return null;
  }
}

export function openTerminalLink(uri: string): void {
  const href = safeLinkTarget(uri);
  if (href) window.open(href, '_blank', 'noopener,noreferrer');
}

interface Tooltip {
  hover: (event: MouseEvent, uri: string) => void;
  leave: () => void;
}

/** Hover and leave callbacks that show a link's address as the terminal's tooltip. */
export function linkTooltip(term: Terminal): Tooltip {
  return {
    hover: (_event, uri) => { if (term.element) term.element.title = uri; },
    leave: () => { term.element?.removeAttribute('title'); },
  };
}

/** The handler for OSC 8 links, in place of xterm's own confirm() prompt. */
export function hyperlinkHandler(term: Terminal): ILinkHandler {
  return { activate: (_event, uri) => openTerminalLink(uri), ...linkTooltip(term) };
}
