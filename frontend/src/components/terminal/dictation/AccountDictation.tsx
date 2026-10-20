import React, { useEffect, useState } from 'react';
import DictationButton from './DictationButton';
import { dictationAvailable, openAccountDictation } from './sockets';

interface Props {
  onText: (text: string) => void;
  onKeys: (keys: string) => void;
  variant?: 'head' | 'bar';
}

/** The microphone for a signed-in account's terminal, once dictation is set up. */
export default function AccountDictation(props: Props) {
  const [available, setAvailable] = useState(false);
  useEffect(() => {
    let live = true;
    dictationAvailable().then((yes) => { if (live) setAvailable(yes); });
    return () => { live = false; };
  }, []);
  return available ? <DictationButton open={openAccountDictation} {...props} /> : null;
}
