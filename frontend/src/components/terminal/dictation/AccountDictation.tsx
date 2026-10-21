import React, { useEffect, useState } from 'react';
import DictationButton from './DictationButton';
import { DictationSetup, dictationSetup, openAccountDictation } from './sockets';

interface Props {
  onText: (text: string) => void;
  onKeys: (keys: string) => void;
  variant?: 'head' | 'bar';
}

/** The microphone for a signed-in account's terminal, once dictation is set up. */
export default function AccountDictation(props: Props) {
  const [setup, setSetup] = useState<DictationSetup | null>(null);
  useEffect(() => {
    let live = true;
    dictationSetup().then((s) => { if (live) setSetup(s); });
    return () => { live = false; };
  }, []);
  return setup?.available ? (
    <DictationButton open={openAccountDictation} endpointMs={setup.endpoint_ms} enterOnPause={setup.auto_enter} {...props} />
  ) : null;
}
