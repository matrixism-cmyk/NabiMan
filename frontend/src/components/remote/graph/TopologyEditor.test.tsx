import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { I18nProvider } from '../../../i18n';
import TopologyEditor from './TopologyEditor';
import { buildWorkMap } from './graphModel';
import { apiRequest } from '../../../shared/api/useApi';

jest.mock('../../../shared/api/useApi', () => ({ apiRequest: jest.fn() }));

const topo = { devices: [{ key: 'inet', name: '인터넷', kind: 'internet', address: '', note: '' }, { key: 'fw', name: '방화벽', kind: 'firewall', address: '1.2.3.4', note: '' }, { key: 'sw', name: '서비스망', kind: 'switch', address: '', note: '' }],
  links: [{ from: 'inet', to: 'fw', label: '회선' }, { from: 'fw', to: 'sw', label: '' }], hosts: [{ device: 'sw', server_host: 'localhost' }] };

it('devices stand in tiers counted from the internet', () => {
  const g = buildWorkMap([], [], [], [], '이 서버', topo);
  expect(Object.fromEntries(g.nodes.filter((n) => n.kind === 'device').map((n) => [n.id, n.layer]))).toEqual({ 'dev:inet': 0, 'dev:fw': 1, 'dev:sw': 2 });
});

it('edits devices, drops a device with its links and ties, and saves the whole document', async () => {
  localStorage.setItem('nabiman_lang', 'ko');
  (apiRequest as jest.Mock).mockResolvedValue({ success: true, data: { devices: [], links: [], hosts: [] } });
  const saved = jest.fn(); const close = jest.fn();
  render(<I18nProvider><TopologyEditor topology={topo} serverHosts={['localhost']} onSaved={saved} onClose={close} /></I18nProvider>);
  expect(screen.getByRole('tab', { name: '장비 3' })).toBeInTheDocument();
  fireEvent.change(screen.getAllByLabelText('이름')[1], { target: { value: 'AXGATE' } });
  // Removing the switch takes its link and the server tied to it.
  fireEvent.click(screen.getAllByRole('button', { name: '삭제' })[2]);
  expect(screen.getByRole('tab', { name: '연결 1' })).toBeInTheDocument();
  expect(screen.getByRole('tab', { name: '서버 위치 0' })).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '저장' }));
  await waitFor(() => expect(saved).toHaveBeenCalled());
  const [path, opts] = (apiRequest as jest.Mock).mock.calls[0];
  expect(path).toBe('/api/terminal/topology');
  expect(JSON.parse(opts.body)).toEqual({ devices: [topo.devices[0], { ...topo.devices[1], name: 'AXGATE' }], links: [topo.links[0]], hosts: [] });
  expect(close).toHaveBeenCalled();
});
