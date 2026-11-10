import { injectFeedbackWidget, loadBranding, forgetBranding } from './branding';
import { apiRequest as request } from '../api/useApi';

jest.mock('../api/useApi', () => ({ apiRequest: jest.fn() }));
const apiRequest = request as jest.Mock;

afterEach(() => { document.body.innerHTML = ''; forgetBranding(); apiRequest.mockReset(); });

test('no widget configured: nothing is added to the page', () => {
  expect(injectFeedbackWidget(null)).toBe(false);
  expect(document.querySelectorAll('script').length).toBe(0);
});

test('a configured widget is added once, with its origin and site', () => {
  const widget = { script_url: '/fixhub-admin.js', origin: 'https://feedback.example', site: 'site-1' };
  expect(injectFeedbackWidget(widget)).toBe(true);
  expect(injectFeedbackWidget(widget)).toBe(false);
  const scripts = document.querySelectorAll('script');
  expect(scripts.length).toBe(1);
  expect(scripts[0].getAttribute('src')).toBe('/fixhub-admin.js');
  expect(scripts[0].getAttribute('data-origin')).toBe('https://feedback.example');
  expect(scripts[0].getAttribute('data-site')).toBe('site-1');
});

test('branding is asked once and a failure is asked again', async () => {
  apiRequest.mockResolvedValueOnce({ success: false, data: null, message: 'no' });
  expect(await loadBranding()).toBeNull();
  const data = { product_name: 'NabiMan', feedback_widget: null, noc_contact: null };
  apiRequest.mockResolvedValue({ success: true, data, message: '' });
  expect(await loadBranding()).toEqual(data);
  expect(await loadBranding()).toEqual(data);
  expect(apiRequest).toHaveBeenCalledTimes(2);
});
