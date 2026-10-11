/* Locally controlled feedback launcher. No external script runs in NabiMan's document.
 * The optional panel is loaded only after a verified administrator clicks the button. */
(function () {
  'use strict';
  var origin = 'https://fix.bebo.kr';
  var site = 'fs_4oAGj2jt6Dgzp61MFo0uTq7_';
  var checked = null, actor = null, button = null, panel = null, frame = null;
  function token() { try { return sessionStorage.getItem('nabiman_token'); } catch (_) { return null; } }
  function close() { if (panel) panel.remove(); panel = null; frame = null; if (button) button.setAttribute('aria-expanded', 'false'); }
  function reset() { close(); if (button) button.remove(); button = null; actor = null; }
  function label(ko, en, ja) {
    var lang = localStorage.getItem('nabiman_lang'); return lang === 'en' ? en : lang === 'ja' ? ja : ko;
  }
  async function administrator(value) {
    if (!value) return null;
    try {
      var response = await fetch('/api/users/me', { headers: { Authorization: 'Bearer ' + value }, cache: 'no-store' });
      if (!response.ok || value !== token()) return null;
      var result = await response.json();
      return value === token() && result.success && result.data && result.data.role === 'admin' && result.data.status === 'active' ? result.data.id : null;
    } catch (_) { return null; }
  }
  async function open() {
    var value = token(), id = await administrator(value);
    if (!id || id !== actor || value !== token()) return reset();
    if (panel) return close();
    panel = document.createElement('section');
    panel.setAttribute('data-nabiman-feedback-panel', '');
    Object.assign(panel.style, { position: 'fixed', right: '16px', bottom: '72px', width: 'min(420px, calc(100vw - 32px))', height: 'min(650px, calc(100dvh - 100px))', zIndex: '2147483000', display: 'flex', flexDirection: 'column', background: '#fff', border: '1px solid #aaa', borderRadius: '12px', overflow: 'hidden' });
    var dismiss = document.createElement('button');
    dismiss.type = 'button'; dismiss.textContent = label('외부 개선 리포트 닫기', 'Close external feedback', '外部フィードバックを閉じる');
    dismiss.addEventListener('click', close);
    frame = document.createElement('iframe');
    frame.title = label('외부 개선 리포트', 'External feedback', '外部フィードバック');
    frame.referrerPolicy = 'no-referrer';
    frame.setAttribute('sandbox', 'allow-scripts allow-forms allow-same-origin allow-popups');
    Object.assign(frame.style, { width: '100%', flex: '1', border: '0' });
    frame.src = origin + '/panel?site=' + encodeURIComponent(site);
    panel.append(dismiss, frame); document.body.appendChild(panel); button.setAttribute('aria-expanded', 'true');
  }
  async function check() {
    var value = token();
    if (!value) { checked = null; return reset(); }
    if (value === checked) return;
    checked = value;
    var id = await administrator(value);
    if (value !== token()) return;
    if (!id) { checked = null; return reset(); }
    if (actor && actor !== id) reset();
    actor = id;
    if (!button) {
      button = document.createElement('button'); button.type = 'button'; button.className = 'btn btn-secondary';
      button.setAttribute('data-nabiman-feedback-launcher', ''); button.setAttribute('aria-expanded', 'false');
      button.textContent = label('개선 리포트 (외부)', 'Feedback (external)', '改善レポート（外部）');
      Object.assign(button.style, { position: 'fixed', right: '16px', bottom: '16px', zIndex: '2147483000' });
      button.addEventListener('click', open); document.body.appendChild(button);
    }
  }
  window.addEventListener('message', function (event) {
    if (!frame || event.origin !== origin || event.source !== frame.contentWindow || !event.data) return;
    if (event.data.type === 'fixhub:close') return close();
    if (event.data.type === 'fixhub:ready' || event.data.type === 'fixhub:context?') {
      // Fixed public product context only. Never forward DOM text, URL parameters,
      // terminal contents, credentials, report JSON, or caller-supplied app context.
      frame.contentWindow.postMessage({ type: 'fixhub:context', page: { page: '/', query: '', page_title: 'NabiMan', context: {} } }, origin);
    } else if (event.data.type === 'fixhub:pick') {
      frame.contentWindow.postMessage({ type: 'fixhub:picked', selector: null, text: null }, origin);
    }
  });
  window.addEventListener('nabiman_auth_changed', function () { checked = null; reset(); check(); });
  setInterval(check, 5000); check();
})();
