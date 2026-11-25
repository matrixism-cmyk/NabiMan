import { fileLinkUrl, openedFileLink, parseFileLink } from './fileLink';

describe('a link to a file', () => {
  const link = { scope: 'local', path: '/workspace/5gmec/작업본/deck/강의 자료 #1.pptx' };

  it('names the server and the path, and nothing else', () => {
    const url = fileLinkUrl(link, 'https://nabi.example');
    expect(url).toBe(`https://nabi.example/remote#file/local/workspace/5gmec/${encodeURIComponent('작업본')}/deck/${encodeURIComponent('강의 자료 #1.pptx')}`);
    expect(url).not.toMatch(/token|key|ticket|[?&]/i);
    const parsed = new URL(url);
    expect(parseFileLink(parsed.pathname, parsed.hash)).toEqual(link);
  });

  it('is not read from other pages or broken addresses', () => {
    expect(parseFileLink('/', '#file/local/etc/hosts')).toBeNull();
    expect(parseFileLink('/remote', '#console/vm1')).toBeNull();
    expect(parseFileLink('/remote', '#file/local')).toBeNull();
    expect(parseFileLink('/remote', '#file//etc/hosts')).toBeNull();
    expect(parseFileLink('/remote', '#file/a b/etc/hosts')).toBeNull();
    expect(parseFileLink('/remote', '#file/local/%E0%A4%A')).toBeNull();
    expect(parseFileLink('/remote/', '#file/srv-1/etc/hosts')).toEqual({ scope: 'srv-1', path: '/etc/hosts' });
  });

  it('survives a sign-in that leaves the page', () => {
    window.history.replaceState(null, '', '/remote#file/local/etc/hosts');
    expect(openedFileLink(false)).toBeNull();
    // Single sign-on comes back to the front page.
    window.history.replaceState(null, '', '/');
    expect(openedFileLink(true)).toEqual({ scope: 'local', path: '/etc/hosts' });
    expect(window.location.pathname + window.location.hash).toBe('/remote#file/local/etc/hosts');
    // Used once: the next visit to the front page is just that.
    window.history.replaceState(null, '', '/');
    expect(openedFileLink(true)).toBeNull();
  });
});
