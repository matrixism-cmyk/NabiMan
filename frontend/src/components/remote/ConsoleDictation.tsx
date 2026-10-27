import React from 'react';
import AccountDictation from '../terminal/dictation/AccountDictation';
import DictationButton from '../terminal/dictation/DictationButton';
import { openConsoleShareDictation } from '../terminal/dictation/sockets';
import { ConsoleShare, ConsoleViewPrefs, consoleDictationProps } from './consoleView';

/**
 * The microphone on a VM console's bar: a signed-in account's own dictation,
 * or — on a share link that lets its visitor control the VM — the link's.
 */
export default function ConsoleDictation({ share, view, onText, onKeys }: {
  share?: ConsoleShare;
  view: ConsoleViewPrefs;
  onText: (text: string) => void;
  onKeys: (keys: string) => void;
}) {
  if (!share) return <AccountDictation variant="bar" onText={onText} onKeys={onKeys} {...consoleDictationProps(view)} />;
  const own = share.dictation;
  if (share.viewOnly || !own) return null;
  return (
    <DictationButton variant="bar" open={() => openConsoleShareDictation(share.token, share.pw)}
      endpointMs={view.dictation_pause_ms ?? own.pause_ms} enterOnPause={view.dictation_auto_enter ?? own.auto_enter}
      onText={onText} onKeys={onKeys} />
  );
}
