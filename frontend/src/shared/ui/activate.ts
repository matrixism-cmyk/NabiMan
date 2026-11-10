import type React from 'react';

/**
 * A key handler doing what a click does, for something that is not a button
 * but acts like one (a sortable column, a table row, a card): Enter or Space
 * runs it, as they would a button. Keys typed in a field inside are left alone.
 */
export function onActivateKey(handler: () => void) {
  return (e: React.KeyboardEvent) => {
    if (e.target !== e.currentTarget) return;
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handler(); }
  };
}

/** Everything a clickable non-button element needs to be reached and used from the keyboard. */
export function clickable(handler: () => void, role: 'button' | 'link' | 'option' = 'button') {
  return { role, tabIndex: 0, onClick: handler, onKeyDown: onActivateKey(handler) };
}
