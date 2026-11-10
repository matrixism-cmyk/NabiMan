import { speakerLabel } from './speakerLabel';
import voices from '../../i18n/translations/claude-voices';

const SERVER_IDS = ['sohee', 'vivian', 'serena', 'ono_anna', 'aiden', 'ryan', 'uncle_fu', 'dylan', 'eric'];

test('every voice the server offers is described in each language', () => {
  for (const id of SERVER_IDS) {
    const entry = voices[`claude.speaker.${id}`];
    expect(entry).toBeDefined();
    expect(entry.ko && entry.en && entry.ja).toBeTruthy();
  }
});

test('the label is the name and the translated description, or only the name', () => {
  const t = (key: string) => voices[key]?.en ?? key;
  expect(speakerLabel({ id: 'aiden', name: 'Aiden' }, t)).toBe('Aiden — Bright American male (English)');
  expect(speakerLabel({ id: 'new_voice', name: 'New' }, t)).toBe('New');
});
