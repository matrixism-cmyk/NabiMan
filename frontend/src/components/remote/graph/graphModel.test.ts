import { buildGraph, depths, groupOf, readView, saveView } from './graphModel';

const s = (id: string, label: string, extra: object = {}) => ({ id, label, owner: 'a', ws_count: 0, shared: false, age_secs: 1, timeout_secs: 0, scrollback: 1, alive: true, title: '', preview: '', ...extra }) as any;

it('groups sessions by where they run', () => {
  expect(groupOf('local')).toBe('local');
  expect(groupOf('ssh:jcia@spark.bebo.kr:22')).toBe('spark.bebo.kr');
  expect(groupOf('telnet:10.0.0.5:23')).toBe('10.0.0.5');
});

it('turns sessions and hand-offs into nodes and edges, the lead in the middle', () => {
  const sessions = [s('lead', 'local', { title: 'nabiman', lead: true, claude: [{ name: 'nabiman-5d' }] }), s('a', 'local', { busy: true }), s('b', 'ssh:x@spark:22'), s('c', 'local')];
  const links = [{ from: 'lead', to: 'a', orchestrator: true, live: true, strength: 1, note: 'n' }, { from: 'a', to: 'b', orchestrator: false, live: false, strength: 0.4 }, { from: 'lead', to: 'gone', orchestrator: true, live: false }];
  const g = buildGraph(sessions, links as any);
  expect(g.nodes.map((n) => [n.id, n.name, n.group, n.busy, n.lead])).toEqual([['lead', 'nabiman', 'local', false, true], ['a', 'local', 'local', true, false], ['b', 'spark', 'spark', false, false], ['c', 'local', 'local', false, false]]);
  // One colour per server; an arrow to something that is no session is dropped.
  expect(g.nodes[0].color).toBe(g.nodes[1].color);
  expect(g.nodes[2].color).not.toBe(g.nodes[0].color);
  expect(g.edges.map((e) => e.id)).toEqual(['lead>a', 'a>b']);
  expect(g.edges[0].color).not.toBe('');
  const d = depths(g);
  expect([d.get('lead'), d.get('a'), d.get('b'), d.get('c')]).toEqual([0, 1, 2, undefined]);
});

it('remembers the chosen view', () => {
  localStorage.clear();
  expect(readView()).toBe('tiles');
  saveView('graph3d');
  expect(readView()).toBe('graph3d');
  localStorage.setItem('nabiman_sessions_view', 'nonsense');
  expect(readView()).toBe('tiles');
});

it('the work map adds the machines sessions run on, what each is doing and how busy it is', () => {
  const { buildWorkMap } = require('./graphModel');
  const sessions = [s('lead', 'local', { title: 'nabiman', lead: true }), s('a', 'ssh:jcia@spark.bebo.kr:22', { busy: true }), s('b', 'ssh:x@10.0.0.9:2222')];
  const links = [{ from: 'lead', to: 'a', orchestrator: true, live: true, strength: 1 }];
  const servers = [{ id: 's1', name: 'nvidia', host: 'spark.bebo.kr', port: 22, status: 'online' }];
  const events = [{ id: 1, at: 10, from: 'lead', to: 'a', from_name: 'l', to_name: 'a', text: '그림 갱신' }, { id: 2, at: 20, from: 'a', to: 'lead', from_name: 'a', to_name: 'l', text: '완료' }];
  const g = buildWorkMap(sessions, links as any, servers, events, '이 서버');
  const by = Object.fromEntries(g.nodes.map((n: any) => [n.id, n]));
  expect(by['host:local']).toEqual(expect.objectContaining({ kind: 'server', name: '이 서버' }));
  expect(by['srv:s1']).toEqual(expect.objectContaining({ kind: 'server', name: 'nvidia', sub: 'spark.bebo.kr' }));
  expect(by['host:10.0.0.9']).toEqual(expect.objectContaining({ kind: 'server', name: '10.0.0.9' }));
  expect(by.a).toEqual(expect.objectContaining({ kind: 'session', task: '그림 갱신', heat: 2 }));
  expect(g.edges.map((e: any) => `${e.kind || 'order'}:${e.id}`).sort()).toEqual(['conn:conn>host:10.0.0.9', 'conn:conn>srv:s1', 'host:host>a', 'host:host>b', 'host:host>lead', 'order:lead>a']);
});
