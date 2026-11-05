import { hyperlinkHandler, linkTooltip, openTerminalLink, safeLinkTarget } from './terminalLinks';

describe('terminal links', () => {
  afterEach(() => jest.restoreAllMocks());

  it('opens only web addresses', () => {
    expect(safeLinkTarget('https://claude.ai/code?x=1')).toBe('https://claude.ai/code?x=1');
    expect(safeLinkTarget('http://example.com')).toBe('http://example.com/');
    for (const bad of ['javascript:alert(1)', 'file:///etc/passwd', 'data:text/html,hi', 'not a url', '']) {
      expect(safeLinkTarget(bad)).toBeNull();
    }
  });

  it('opens a link in a new tab that cannot reach back, and nothing else', () => {
    const open = jest.spyOn(window, 'open').mockReturnValue(null);
    openTerminalLink('https://example.com/a');
    expect(open).toHaveBeenCalledWith('https://example.com/a', '_blank', 'noopener,noreferrer');
    openTerminalLink('javascript:alert(1)');
    expect(open).toHaveBeenCalledTimes(1);
  });

  it('opens an OSC 8 link without asking, and shows where it goes on hover', () => {
    const open = jest.spyOn(window, 'open').mockReturnValue(null);
    const confirm = jest.spyOn(window, 'confirm');
    const element = document.createElement('div');
    const term = { element } as never;
    const handler = hyperlinkHandler(term);
    const range = { start: { x: 1, y: 1 }, end: { x: 5, y: 1 } };
    handler.hover?.(new MouseEvent('mousemove'), 'https://example.com/real', range);
    expect(element.title).toBe('https://example.com/real');
    handler.activate(new MouseEvent('mouseup'), 'https://example.com/real', range);
    expect(open).toHaveBeenCalledWith('https://example.com/real', '_blank', 'noopener,noreferrer');
    expect(confirm).not.toHaveBeenCalled();
    handler.leave?.(new MouseEvent('mouseleave'), 'https://example.com/real', range);
    expect(element.hasAttribute('title')).toBe(false);
    // Before the terminal is on the page there is nowhere to show it: no error.
    expect(() => linkTooltip({ element: undefined } as never).hover(new MouseEvent('mousemove'), 'x')).not.toThrow();
  });
});
