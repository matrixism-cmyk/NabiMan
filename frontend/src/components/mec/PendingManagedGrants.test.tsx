import React from 'react';
import { render, screen } from '@testing-library/react';
import PendingManagedGrants from './PendingManagedGrants';
import { useMecList } from '../../hooks/mec/useMecApi';
import { I18nProvider } from '../../i18n';
jest.mock('../../hooks/mec/useMecApi', () => ({ useMecList: jest.fn() }));
const row = (binding: string | null) => ({ local_user_id: 'local-member', source_role: 'viewer', operation: { id: 'request-one', realm: 'https://rancher.example', project_id: 'project-one', remote_user_id: 'remote-one', role_template: 'read-only', binding_id: binding, state: 'unknown_result' } });
const panel = () => <I18nProvider><PendingManagedGrants tenantId="tenant/one" /></I18nProvider>;
beforeEach(() => localStorage.setItem('nabiman_lang', 'ko'));
it('shows a missing durable identity as requiring review without inferring revocation', () => {
  (useMecList as jest.Mock).mockReturnValue({ data: [row(null)], error: null, loading: false, refetch: jest.fn() });
  render(panel());
  expect(screen.getByText(/자동 확인·회수가 불가능합니다/)).toBeInTheDocument();
  expect(screen.getByText(/현재 유효 접근 권한을 증명하지 않습니다/)).toBeInTheDocument();
  expect(useMecList).toHaveBeenCalledWith('/api/mec/v1/tenants/tenant%2Fone/pending-managed-grants', 30_000);
});
it('keeps a known candidate uncertain until exact observation and completed cleanup', () => {
  (useMecList as jest.Mock).mockReturnValue({ data: [row('binding-one')], error: null, loading: false, refetch: jest.fn() });
  render(panel());
  expect(screen.getByText('binding-one')).toBeInTheDocument();
  expect(screen.getByText(/회수가 완료된 상태는 아닙니다/)).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: /회수|생성/ })).not.toBeInTheDocument();
});
it('hides previous observations after refresh failure', () => {
  (useMecList as jest.Mock).mockReturnValue({ data: [row('binding-one')], error: null, loading: false, refetch: jest.fn() });
  const { rerender } = render(panel());
  expect(screen.getByText('binding-one')).toBeInTheDocument();
  (useMecList as jest.Mock).mockReturnValue({ data: [row('binding-one')], error: 'Unavailable', loading: false, refetch: jest.fn() });
  rerender(panel());
  expect(screen.queryByText('binding-one')).not.toBeInTheDocument();
  expect(screen.getByText('Unavailable')).toBeInTheDocument();
});
