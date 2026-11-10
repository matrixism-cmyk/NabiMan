import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { I18nProvider } from '../../i18n';
import { apiRequest } from '../../shared/api/useApi';
jest.mock('../../shared/api/useApi',()=>({apiRequest:jest.fn(),apiPost:jest.fn()}));
import SessionsView, { SessionInfo } from './SessionsView';
const session: SessionInfo = { id:'pty_one',label:'local',owner:'admin',ws_count:0,shared:false,age_secs:10,timeout_secs:300,scrollback:2000,keepalive:false,alive:true,title:'Fixture',preview:'' };
function setup(value: SessionInfo) {
  localStorage.setItem('nabiman_lang','ko');
  const attach=jest.fn();
  render(<I18nProvider><SessionsView sessions={[value]} onRefresh={jest.fn()} onAttach={attach} onPopOut={jest.fn()} onShare={jest.fn()}/></I18nProvider>);
  return attach;
}
it('shows unavailable state separately and disables connection actions',()=>{
  const attach=setup({...session,alive:false,state:'unknown'});
  expect(screen.getByText('상태 확인 불가')).toBeInTheDocument();
  const join=screen.getByRole('button',{name:'참여'});
  expect(join).toBeDisabled();fireEvent.click(join);expect(attach).not.toHaveBeenCalled();
});
it('does not allow an approval-blocked session even if a stale alive flag is true',()=>{
  setup({...session,state:'blocked'});
  expect(screen.getByText('접근 승인 변경')).toBeInTheDocument();
  expect(screen.getByRole('button',{name:'참여'})).toBeDisabled();
});
it('preserves attach behavior for a confirmed live session',()=>{
  const attach=setup({...session,state:'alive'});
  fireEvent.click(screen.getByRole('button',{name:'참여'}));expect(attach).toHaveBeenCalledWith({...session,state:'alive'});
});

it('reports an uncertain network outcome without retrying termination',async()=>{
  jest.spyOn(window,'confirm').mockReturnValue(true);
  const req=apiRequest as jest.Mock;
  req.mockImplementation((path:string,opts?:RequestInit)=>opts?.method==='DELETE'
    ?Promise.reject(new Error('Connection closed')):Promise.resolve({success:true,data:{mode:'newest',ids:[]}}));
  setup({...session,state:'alive'});
  fireEvent.click(screen.getByRole('button',{name:'세션 종료'}));
  await waitFor(()=>expect(screen.getByRole('alert')).toHaveTextContent('요청 결과를 확인하지 못했습니다'));
  expect(req.mock.calls.filter(([,opts])=>opts?.method==='DELETE')).toHaveLength(1);
  req.mockReset();
  jest.restoreAllMocks();
});

it('a session whose Claude is working blinks, with its address to copy',()=>{
  const { container } = render(<I18nProvider><SessionsView sessions={[{...session,state:'alive',busy:true,
    claude:[{name:'nabiman-13',ref:'df126b',address:'nabiman-13 [df126b]',cwd:'/workspace/nabiman',busy:true}]}]}
    onRefresh={jest.fn()} onAttach={jest.fn()} onPopOut={jest.fn()}/></I18nProvider>);
  expect(container.querySelector('.session-card')).toHaveClass('is-working');
  expect(screen.getByText('Claude 작업 중')).toBeInTheDocument();
  expect(screen.getByRole('button',{name:'Claude 세션 주소 nabiman-13 [df126b] 복사'})).toBeInTheDocument();
});

it('a screen that changed since the last refresh glows, and an unchanged one does not',()=>{
  localStorage.setItem('nabiman_lang','ko');
  const view=(preview:string)=>(<I18nProvider><SessionsView sessions={[{...session,state:'alive',preview}]}
    onRefresh={jest.fn()} onAttach={jest.fn()} onPopOut={jest.fn()}/></I18nProvider>);
  const { container, rerender } = render(view('$ make'));
  const card=()=>container.querySelector('.session-card');
  expect(card()).not.toHaveClass('is-output');
  rerender(view('$ make\nbuilding…'));
  expect(card()).toHaveClass('is-output');
  expect(screen.getByText('출력 중')).toBeInTheDocument();
  expect(card()).not.toHaveClass('is-working');
});

it('the orchestrator blinks red and an arrow runs to each session it handed work to',()=>{
  localStorage.setItem('nabiman_lang','ko');
  const peer=(name:string)=>[{name,ref:'abcdef',address:`${name} [abcdef]`,cwd:'/w',busy:false}];
  const art:SessionInfo={...session,id:'pty_art',title:'artisland',state:'alive',busy:true,claude:peer('artisland-bd'),orchestrator:true,directs:['pty_nabi','pty_mpcf','pty_gone']};
  const nabi:SessionInfo={...session,id:'pty_nabi',title:'nabiman',state:'alive',busy:true,claude:peer('nabiman-13')};
  const mpcf:SessionInfo={...session,id:'pty_mpcf',title:'mpcf',state:'alive',claude:peer('mpcf-18')};
  const { container } = render(<I18nProvider><SessionsView sessions={[art,nabi,mpcf]}
    onRefresh={jest.fn()} onAttach={jest.fn()} onPopOut={jest.fn()}/></I18nProvider>);
  const card=(id:string)=>container.querySelector(`[data-session-id="${id}"]`);
  expect(card('pty_art')).toHaveClass('is-orchestrator');
  expect(card('pty_art')).not.toHaveClass('is-working');
  expect(screen.getByText('지휘 중 · 세션 2개')).toBeInTheDocument();
  // A worker keeps its own green and says who it works for.
  expect(card('pty_nabi')).toHaveClass('is-working');
  expect(screen.getAllByText('artisland-bd의 지시')).toHaveLength(2);
  // Only sessions on this list are joined.
  expect(Array.from(container.querySelectorAll('.session-link')).map((p)=>p.getAttribute('data-link')).sort())
    .toEqual(['pty_art>pty_mpcf','pty_art>pty_nabi']);
});

it('the handle moves a tile with the arrow keys and saves the order; the sort can be chosen',async()=>{
  localStorage.setItem('nabiman_lang','ko');
  const req=apiRequest as jest.Mock;
  req.mockReset();
  req.mockImplementation((path:string,opts?:RequestInit)=>Promise.resolve(opts?.method==='PUT'
    ?{success:true,data:JSON.parse(String(opts.body))}:{success:true,data:{mode:'newest',ids:[]}}));
  const a:SessionInfo={...session,id:'pty_a',title:'가',age_secs:1,state:'alive'};
  const b:SessionInfo={...session,id:'pty_b',title:'나',age_secs:2,state:'alive'};
  const { container } = render(<I18nProvider><SessionsView sessions={[a,b]} onRefresh={jest.fn()} onAttach={jest.fn()} onPopOut={jest.fn()}/></I18nProvider>);
  const order=()=>Array.from(container.querySelectorAll('[data-session-id]')).map((c)=>c.getAttribute('data-session-id'));
  await waitFor(()=>expect(req).toHaveBeenCalledWith('/api/terminal/session-order'));
  expect(order()).toEqual(['pty_a','pty_b']);
  fireEvent.keyDown(screen.getByRole('button',{name:'나 옮기기'}),{key:'ArrowLeft'});
  expect(order()).toEqual(['pty_b','pty_a']);
  expect(req).toHaveBeenCalledWith('/api/terminal/session-order',{method:'PUT',body:JSON.stringify({mode:'custom',ids:['pty_b','pty_a']})});
  const sort=screen.getByRole('combobox',{name:'세션 정렬'});
  expect(sort).toHaveValue('custom');
  fireEvent.change(sort,{target:{value:'name'}});
  expect(order()).toEqual(['pty_a','pty_b']);
});

it('when the orchestrator and its workers are waiting, the arrows fade and it stops blinking',()=>{
  localStorage.setItem('nabiman_lang','ko');
  const peer=(name:string)=>[{name,ref:'abcdef',address:`${name} [abcdef]`,cwd:'/w',busy:false}];
  const art:SessionInfo={...session,id:'pty_art',title:'artisland',state:'alive',busy:false,claude:peer('artisland-bd'),orchestrator:true,directs:['pty_nabi','pty_mpcf']};
  const nabi:SessionInfo={...session,id:'pty_nabi',title:'nabiman',state:'alive',busy:true,claude:peer('nabiman-13')};
  const mpcf:SessionInfo={...session,id:'pty_mpcf',title:'mpcf',state:'alive',claude:peer('mpcf-18')};
  const view=(list:SessionInfo[])=>(<I18nProvider><SessionsView sessions={list} onRefresh={jest.fn()} onAttach={jest.fn()} onPopOut={jest.fn()}/></I18nProvider>);
  const { container, rerender } = render(view([art,nabi,mpcf]));
  const link=(k:string)=>container.querySelector(`[data-link="${k}"]`);
  const card=()=>container.querySelector('[data-session-id="pty_art"]');
  // nabiman still works on what it was handed: that arrow stays, the idle one fades.
  expect(link('pty_art>pty_nabi')).not.toHaveClass('is-faint');
  expect(link('pty_art>pty_mpcf')).toHaveClass('is-faint');
  expect(card()).not.toHaveClass('is-resting');
  rerender(view([art,{...nabi,busy:false},mpcf]));
  expect(link('pty_art>pty_nabi')).toHaveClass('is-faint');
  expect(card()).toHaveClass('is-orchestrator');
  expect(card()).toHaveClass('is-resting');
});

it('Claude on another server: its tile shows @server, marked unreachable without Remote Control',()=>{
  localStorage.setItem('nabiman_lang','ko');
  const tile:SessionInfo={...session,id:'pty_ssh',label:'ssh:Administrator@172.20.26.163:22',title:'winsrv',state:'alive',
    claude:[{name:'worker-1',ref:'',address:'worker-1',cwd:'C:\\w',busy:false,server:'winsrv'}]};
  render(<I18nProvider><SessionsView sessions={[tile]} onRefresh={jest.fn()} onAttach={jest.fn()} onPopOut={jest.fn()}/></I18nProvider>);
  expect(screen.getByText('@winsrv')).toBeInTheDocument();
  expect(screen.getByRole('button',{name:/worker-1/})).toHaveClass('is-unreachable');
});

it('only sessions are tiles, and one target\'s list still badges work handed to a session elsewhere',()=>{
  localStorage.setItem('nabiman_lang','ko');
  const peer=(name:string)=>[{name,ref:'abcdef',address:`${name} [abcdef]`,cwd:'/w',busy:false}];
  const five:SessionInfo={...session,id:'pty_5g',label:'local',title:'5gmec',state:'alive',claude:peer('5gmec-9a')};
  const lead:SessionInfo={...session,id:'pty_lead',label:'ssh:a@b:22',title:'lead',state:'alive',busy:true,orchestrator:true,claude:peer('lead-1'),directs:['pty_5g','lc:gone']};
  const { container } = render(<I18nProvider><SessionsView sessions={[five]} context={[five,lead]} onRefresh={jest.fn()} onAttach={jest.fn()} onPopOut={jest.fn()}/></I18nProvider>);
  expect(Array.from(container.querySelectorAll('[data-session-id]')).map((c)=>c.getAttribute('data-session-id'))).toEqual(['pty_5g']);
  // The leader is on another target: no arrow here, but the badge says who leads.
  expect(container.querySelectorAll('.session-link')).toHaveLength(0);
  expect(screen.getByText('lead-1의 지시')).toBeInTheDocument();
  // Work handed to something that is no session draws nothing.
  expect(container.querySelector('[data-session-id="lc:gone"]')).toBeNull();
});

it('an arrow is strong only while the work is being done, and fades as time passes after it',()=>{
  localStorage.setItem('nabiman_lang','ko');
  jest.spyOn(Date,'now').mockReturnValue(1_800_000_000_000);
  const now=1_800_000_000;
  const peer=(name:string,since?:number)=>[{name,ref:'abcdef',address:`${name} [abcdef]`,cwd:'/w',busy:false,since}];
  const lead:SessionInfo={...session,id:'pty_l',title:'lead',state:'alive',claude:peer('lead'),orchestrator:true,directs:['pty_w','pty_d1','pty_d2']};
  const working:SessionInfo={...session,id:'pty_w',title:'w',state:'alive',busy:true,claude:[{...peer('w')[0],busy:true}]};
  const justDone:SessionInfo={...session,id:'pty_d1',title:'d1',state:'alive',claude:peer('d1',now-60)};
  const longDone:SessionInfo={...session,id:'pty_d2',title:'d2',state:'alive',claude:peer('d2',now-3*3600)};
  const { container } = render(<I18nProvider><SessionsView sessions={[lead,working,justDone,longDone]} onRefresh={jest.fn()} onAttach={jest.fn()} onPopOut={jest.fn()}/></I18nProvider>);
  const strength=(k:string)=>Number(container.querySelector(`[data-link="${k}"]`)?.parentElement?.getAttribute('data-link-strength'));
  expect(strength('pty_l>pty_w')).toBe(1);
  expect(container.querySelector('[data-link="pty_l>pty_w"]')?.parentElement).toHaveClass('is-live');
  expect(strength('pty_l>pty_d1')).toBeGreaterThan(0.7);
  expect(strength('pty_l>pty_d2')).toBeCloseTo(0.35);
  expect(container.querySelector('[data-link="pty_l>pty_d1"]')?.parentElement).not.toHaveClass('is-live');
  jest.restoreAllMocks();
});

it('pointing at an arrow shows a box with who handed what to whom, and the others step back',()=>{
  localStorage.setItem('nabiman_lang','ko');
  jest.spyOn(Date,'now').mockReturnValue(1_800_000_000_000);
  const peer=(name:string)=>[{name,ref:'abcdef',address:`${name} [abcdef]`,cwd:'/w',busy:false}];
  const lead:SessionInfo={...session,id:'pty_l',title:'lead',state:'alive',claude:peer('lead'),orchestrator:true,directs:['pty_a','pty_b'],
    notes:[{to:'pty_a',text:'보고서 그림 갱신해 줘',at:1_800_000_000-120}]};
  const a:SessionInfo={...session,id:'pty_a',title:'a',state:'alive',claude:peer('worker-a')};
  const b:SessionInfo={...session,id:'pty_b',title:'b',state:'alive',claude:peer('worker-b')};
  const { container } = render(<I18nProvider><SessionsView sessions={[lead,a,b]} onRefresh={jest.fn()} onAttach={jest.fn()} onPopOut={jest.fn()}/></I18nProvider>);
  // No browser tooltip any more.
  expect(container.querySelector('.session-link-hit title')).toBeNull();
  fireEvent.mouseEnter(container.querySelector('[data-link-hit="pty_l>pty_a"]')!,{clientX:40,clientY:40});
  const tip=screen.getByRole('tooltip');
  expect(tip).toHaveTextContent('lead → worker-a');
  expect(tip).toHaveTextContent('마지막 요청 (2m 전): 보고서 그림 갱신해 줘');
  expect(container.querySelector('.session-links')).toHaveClass('is-hovering');
  expect(container.querySelector('[data-link="pty_l>pty_a"]')?.parentElement).toHaveClass('is-hovered');
  fireEvent.mouseLeave(container.querySelector('[data-link-hit="pty_l>pty_a"]')!);
  expect(screen.queryByRole('tooltip')).toBeNull();
  jest.restoreAllMocks();
});
