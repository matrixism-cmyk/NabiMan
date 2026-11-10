import React from 'react';
import { render, screen } from '@testing-library/react';
import AuditChange from './AuditChange';
import { I18nProvider } from '../i18n';

describe('AuditChange', () => {
  it('shows each changed field old → new and hides secret values', () => {
    render(<I18nProvider><AuditChange change={{
      resource: 'user', id: 'u1', action: 'update',
      fields: [{ field: 'role', old: 'viewer', new: 'operator' }, { field: 'password_hash', secret: true }],
    }} /></I18nProvider>);
    expect(screen.getByText('user')).toBeInTheDocument();
    expect(screen.getByText(/viewer → operator/)).toBeInTheDocument();
    expect(screen.getByText('password_hash')).toBeInTheDocument();
    expect(screen.queryByText(/\$2b\$/)).toBeNull();
  });

  it('renders nothing for an entry without a change', () => {
    const { container } = render(<I18nProvider><AuditChange change={undefined} /></I18nProvider>);
    expect(container.querySelector('.audit-change')).toBeNull();
  });
});
