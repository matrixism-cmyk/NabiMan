import { installPanelZoom, zoomOf, zoomStyle } from './panelZoom';

const wheel = (el: Element, deltaY: number, ctrlKey = true) => {
  const e = new WheelEvent('wheel', { deltaY, ctrlKey, bubbles: true, cancelable: true });
  el.dispatchEvent(e);
  return e;
};

beforeEach(() => { localStorage.clear(); document.body.innerHTML = ''; installPanelZoom(); });

it('Ctrl + wheel over a panel zooms that panel alone and remembers it', () => {
  document.body.innerHTML = '<aside data-zoom-key="sftp"><ul><li id="row">a.txt</li></ul></aside><aside data-zoom-key="rail" id="rail"></aside>';
  const e = wheel(document.getElementById('row')!, -100);
  expect(e.defaultPrevented).toBe(true); // the browser does not zoom the page
  expect(zoomOf('sftp')).toBeCloseTo(1.1);
  // (jsdom drops the CSS zoom property itself; the browser check covers how it looks.)
  expect(zoomOf('rail')).toBe(1);
  expect(zoomStyle('sftp')).toEqual({ zoom: 1.1 });
  wheel(document.getElementById('row')!, 100);
  expect(zoomOf('sftp')).toBe(1);
  expect(localStorage.getItem('nabiman_zoom_sftp')).toBeNull();
});

it('stays within bounds, and leaves terminals, plain wheels and other places alone', () => {
  document.body.innerHTML = '<aside data-zoom-key="rail" id="rail"><div class="xterm" id="term"></div></aside><main id="other"></main>';
  for (let i = 0; i < 20; i += 1) wheel(document.getElementById('rail')!, -100);
  expect(zoomOf('rail')).toBe(1.6);
  expect(wheel(document.getElementById('term')!, -100).defaultPrevented).toBe(false);
  expect(wheel(document.getElementById('rail')!, -100, false).defaultPrevented).toBe(false);
  expect(wheel(document.getElementById('other')!, -100).defaultPrevented).toBe(false);
});
