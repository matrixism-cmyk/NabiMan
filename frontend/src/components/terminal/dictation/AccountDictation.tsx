import React, { useEffect, useState } from 'react';
import DictationButton from './DictationButton';
import { DictationSetup, dictationSetup, openAccountDictation } from './sockets';
import { useSessionEntry } from '../sessionSettings';
import type { useVoiceReply } from './useVoiceReply';

interface Props {
  onText: (text: string) => void;
  onKeys: (keys: string) => void;
  variant?: 'head' | 'bar';
  /** The session dictated into: its own pause and Enter, over the server's. */
  sessionId?: string;
  /** Reads the last answer aloud when it is asked for. */
  reply?: ReturnType<typeof useVoiceReply>;
}

/** The microphone for a signed-in account's terminal, once dictation is set up. */
export default function AccountDictation({ sessionId, ...props }: Props) {
  const [setup, setSetup] = useState<DictationSetup | null>(null);
  useEffect(() => {
    let live = true;
    dictationSetup().then((s) => { if (live) setSetup(s); });
    return () => { live = false; };
  }, []);
  const own = useSessionEntry(sessionId)?.overrides;
  return setup?.available ? (
    <DictationButton open={openAccountDictation} endpointMs={own?.dictation_pause_ms ?? setup.endpoint_ms}
      enterOnPause={own?.dictation_auto_enter ?? setup.auto_enter}
      voiceLock={own?.dictation_voice_lock ?? setup.voice_lock} {...props} />
  ) : null;
}
