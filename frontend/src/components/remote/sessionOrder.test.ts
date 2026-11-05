import { arrange, moved } from './sessionOrder';
import type { SessionInfo } from './SessionsView';

const s = (id: string, age: number, extra: Partial<SessionInfo> = {}): SessionInfo => ({
  id, label: id, owner: 'admin', ws_count: 0, shared: false, age_secs: age, timeout_secs: 0, scrollback: 0,
  keepalive: false, alive: true, title: '', preview: '', ...extra,
});
const ids = (list: SessionInfo[]) => list.map((x) => x.id);

test('sorts by age, name, or work going on', () => {
  const list = [s('b', 30, { title: '나비' }), s('a', 10, { title: '가방' }), s('c', 20, { title: '다리', busy: true })];
  expect(ids(arrange(list, { mode: 'newest', ids: [] }))).toEqual(['a', 'c', 'b']);
  expect(ids(arrange(list, { mode: 'oldest', ids: [] }))).toEqual(['b', 'c', 'a']);
  expect(ids(arrange(list, { mode: 'name', ids: [] }))).toEqual(['a', 'b', 'c']);
  expect(ids(arrange([...list, s('o', 99, { orchestrator: true })], { mode: 'active', ids: [] }))).toEqual(['o', 'c', 'a', 'b']);
});

test('in my own order, sessions not placed yet come first, newest first', () => {
  const list = [s('x', 50), s('y', 40), s('new1', 5), s('new2', 1)];
  expect(ids(arrange(list, { mode: 'custom', ids: ['y', 'x'] }))).toEqual(['new2', 'new1', 'y', 'x']);
});

test('moving a tile keeps the places of sessions this list does not show', () => {
  // All: p (other server), a, q (other server), b, c. Shown here: a, b, c.
  const all = ['p', 'a', 'q', 'b', 'c'];
  expect(moved(all, ['a', 'b', 'c'], 'c', 'a')).toEqual(['p', 'c', 'a', 'b', 'q']);
  expect(moved(all, ['a', 'b', 'c'], 'a', null)).toEqual(['p', 'b', 'c', 'a', 'q']);
  // Nothing saved yet: what is shown becomes the order.
  expect(moved([], ['a', 'b', 'c'], 'b', 'a')).toEqual(['b', 'a', 'c']);
});
