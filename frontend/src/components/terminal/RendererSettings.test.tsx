/**
 * The drawing-method picker: choices, what open windows use, and the speed test.
 */
import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import RendererSettings from './RendererSettings';
import { benchmarkRenderers } from './rendererBench';

jest.mock('./rendererBench', () => ({ benchmarkRenderers: jest.fn() }));
jest.mock('./renderers', () => ({
  rendererStates: () => [{ active: 'webgl' }, { active: 'webgl' }, { active: 'canvas', reason: 'webgl-lost' }],
}));

test('picks a method, shows the open windows and the measured speed', async () => {
  // the project resets mock implementations before each test, so set it here
  (benchmarkRenderers as jest.Mock).mockResolvedValue([
    { requested: 'webgl', active: 'canvas', fps: 41.6, cpuMs: 2.25 },
    { requested: 'canvas', active: 'canvas', fps: 44, cpuMs: 2.5 },
    { requested: 'dom', active: 'dom', fps: 20, cpuMs: null },
  ]);
  const onChange = jest.fn();
  render(<I18nProvider><RendererSettings value="auto" onChange={onChange} fontFamily="d2coding" fontSize={14} /></I18nProvider>);
  fireEvent.click(screen.getByRole('button', { name: 'Canvas' }));
  expect(onChange).toHaveBeenCalledWith('canvas');
  expect(screen.getByText(/WebGL ×2 · Canvas \(/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: /속도|speed/ }));
  await waitFor(() => expect(screen.getByTestId('renderer-bench')).toHaveTextContent(
    'WebGL→Canvas 42fps (CPU 2.3ms) · Canvas 44fps (CPU 2.5ms) · DOM 20fps',
  ));
});
