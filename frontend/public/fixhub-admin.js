/* Locally controlled feedback launcher. No external script runs in NabiMan's document.
 * The optional panel is loaded only after a verified administrator clicks the button. */
(function () {
  'use strict';
  var origin = 'https://fix.bebo.kr';
  var site = 'fs_4oAGj2jt6Dgzp61MFo0uTq7_';
  var checked = null, actor = null, button = null, panel = null, frame = null;
  // Where the launcher sits, as distances from the right and bottom edges; it can be dragged anywhere.
  var POS_KEY = 'nabiman_feedback_pos', EDGE = 8, dragged = false;
  function savedPos() {
    try { var p = JSON.parse(localStorage.getItem(POS_KEY) || 'null'); if (p && isFinite(p.right) && isFinite(p.bottom)) return p; } catch (_) { /* none */ }
    return { right: 16, bottom: 16 };
  }
  function place(pos) {
    if (!button) return;
    var w = button.offsetWidth || 140, h = button.offsetHeight || 36;
    var right = Math.min(Math.max(EDGE, pos.right), Math.max(EDGE, window.innerWidth - w - EDGE));
    var bottom = Math.min(Math.max(EDGE, pos.bottom), Math.max(EDGE, window.innerHeight - h - EDGE));
    button.style.right = right + 'px'; button.style.bottom = bottom + 'px';
    if (panel) placePanel();
    return { right: right, bottom: bottom };
  }
  // The panel opens beside the launcher: above it when there is room, otherwise below.
  function placePanel() {
    var r = button.getBoundingClientRect();
    var right = Math.max(EDGE, Math.min(window.innerWidth - r.right, window.innerWidth - 300));
    panel.style.right = right + 'px';
    if (r.top > window.innerHeight / 2) { panel.style.top = ''; panel.style.bottom = (window.innerHeight - r.top + 8) + 'px'; panel.style.height = 'min(650px, ' + Math.max(240, r.top - 24) + 'px)'; }
    else { panel.style.bottom = ''; panel.style.top = (r.bottom + 8) + 'px'; panel.style.height = 'min(650px, ' + Math.max(240, window.innerHeight - r.bottom - 24) + 'px)'; }
  }
  function draggable(el) {
    var start = null;
    el.style.touchAction = 'none';
    el.addEventListener('pointerdown', function (e) {
      if (e.button > 0) return;
      var r = el.getBoundingClientRect();
      start = { x: e.clientX, y: e.clientY, right: window.innerWidth - r.right, bottom: window.innerHeight - r.bottom, moved: false };
      el.setPointerCapture(e.pointerId);
    });
    el.addEventListener('pointermove', function (e) {
      if (!start) return;
      var dx = e.clientX - start.x, dy = e.clientY - start.y;
      if (!start.moved && Math.abs(dx) + Math.abs(dy) < 6) return;
      start.moved = true; el.style.cursor = 'grabbing';
      place({ right: start.right - dx, bottom: start.bottom - dy });
    });
    function end() {
      if (!start) return;
      if (start.moved) {
        dragged = true; setTimeout(function () { dragged = false; }, 0);
        try { localStorage.setItem(POS_KEY, JSON.stringify({ right: parseFloat(el.style.right), bottom: parseFloat(el.style.bottom) })); } catch (_) { /* private mode */ }
      }
      start = null; el.style.cursor = '';
    }
    el.addEventListener('pointerup', end); el.addEventListener('pointercancel', end);
    // Double-click puts it back in its corner.
    el.addEventListener('dblclick', function () { try { localStorage.removeItem(POS_KEY); } catch (_) { /* private mode */ } place({ right: 16, bottom: 16 }); });
  }
  window.addEventListener('resize', function () { place(savedPos()); });
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
    if (dragged) return; // the end of a drag, not a click
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
    placePanel();
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
      button.title = label('끌어서 옮길 수 있습니다 (두 번 누르면 제자리)', 'Drag to move (double-click to reset)', 'ドラッグで移動（ダブルクリックで元の位置）');
      button.addEventListener('click', open); document.body.appendChild(button);
      draggable(button); place(savedPos());
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
