import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { I18nProvider } from '../../i18n';
import { apiRequest } from '../../hooks/useApi';
jest.mock('../../hooks/useApi',()=>({apiRequest:jest.fn(),apiPost:jest.fn()}));
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
