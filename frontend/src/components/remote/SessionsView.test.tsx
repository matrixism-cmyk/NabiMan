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
  (apiRequest as jest.Mock).mockRejectedValueOnce(new Error('Connection closed'));
  setup({...session,state:'alive'});
  fireEvent.click(screen.getByRole('button',{name:'세션 종료'}));
  await waitFor(()=>expect(screen.getByRole('alert')).toHaveTextContent('요청 결과를 확인하지 못했습니다'));
  expect(apiRequest).toHaveBeenCalledTimes(1);
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
  expect(screen.getByText('오케스트레이터 → 2')).toBeInTheDocument();
  // A worker keeps its own green and says who it works for.
  expect(card('pty_nabi')).toHaveClass('is-working');
  expect(screen.getAllByText('← artisland-bd')).toHaveLength(2);
  // Only sessions on this list are joined.
  expect(Array.from(container.querySelectorAll('.session-link')).map((p)=>p.getAttribute('data-link')).sort())
    .toEqual(['pty_art>pty_mpcf','pty_art>pty_nabi']);
});
