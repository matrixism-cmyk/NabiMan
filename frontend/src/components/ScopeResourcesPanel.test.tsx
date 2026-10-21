import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import ScopeResourcesPanel from './ScopeResourcesPanel';
import { useApi } from '../shared/api/useApi';
import { useMecList } from '../hooks/mec/useMecApi';
import { I18nProvider } from '../i18n';
jest.mock('../shared/api/useApi',()=>({useApi:jest.fn()}));jest.mock('../hooks/mec/useMecApi',()=>({useMecList:jest.fn()}));jest.mock('./mec/DelegatedManagedGrant',()=>({__esModule:true,default:({tenantId}:{tenantId:string})=><p>delegate-context:{tenantId}</p>}));
it('exposes the delegated grant entry only for a current tenant administrator',()=>{localStorage.setItem('nabiman_lang','ko');(useApi as jest.Mock).mockReturnValue({data:{memberships:[{kind:'tenant',scope_id:'own',role:'admin',enabled:true},{kind:'tenant',scope_id:'read',role:'viewer',enabled:true},{kind:'tenant',scope_id:'expired',role:'admin',enabled:true,expires_at:'2000-01-01T00:00:00Z'}]},error:null});(useMecList as jest.Mock).mockReturnValue({data:[{id:'own',display_name:'Own',namespace:'own'},{id:'read',display_name:'Read',namespace:'read'},{id:'expired',display_name:'Expired',namespace:'expired'}],error:null});render(<I18nProvider><ScopeResourcesPanel kind="tenant" /></I18nProvider>);expect(screen.getAllByRole('button',{name:'외부 권한 관리'})).toHaveLength(1);fireEvent.click(screen.getByRole('button',{name:'외부 권한 관리'}));expect(screen.getByText('delegate-context:own')).toBeInTheDocument();});
