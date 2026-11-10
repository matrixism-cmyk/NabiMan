/** A voice's name and, when this page knows it, what it sounds like (`claude.speaker.<id>`). */
export function speakerLabel(v: { id: string; name: string }, t: (key: string) => string): string {
  const key = `claude.speaker.${v.id}`;
  const desc = t(key);
  return desc === key ? v.name : `${v.name} — ${desc}`;
}
