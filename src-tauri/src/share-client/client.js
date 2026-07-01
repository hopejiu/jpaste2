// Client-side behavior for the share server page.
// Icons are referenced through <use href="#..."> symbols defined in the template,
// so no SVG path data is duplicated here.
//
// Debug overlay: every network step and any JS error is surfaced in an on-screen
// panel (bottom-left) so failures on mobile (e.g. Android download) are visible
// without a desktop devtools connection.

// ── debug overlay ──────────────────────────────────────────────────────────────
var DEBUG = { open: false, logs: [] };
function dbg(msg, isErr) {
  DEBUG.logs.push({ t: Date.now(), m: String(msg), e: !!isErr });
  if (DEBUG.logs.length > 80) DEBUG.logs.shift();
  renderDebug();
}
function renderDebug() {
  var box = document.getElementById('dbg');
  if (!box) return;
  var html =
    '<div class="dbg-head"><span class="dbg-title">调试 (' + DEBUG.logs.length + ')</span>' +
    '<button type="button" onclick="dbgToggle()">' + (DEBUG.open ? '收起' : '展开') + '</button>' +
    '<button type="button" onclick="dbgClear()">清空</button></div>';
  if (DEBUG.open) {
    html += '<div class="dbg-body">' + DEBUG.logs.map(function (l) {
      var time = new Date(l.t).toLocaleTimeString();
      return '<div class="dbg-line' + (l.e ? ' err' : '') + '">[' + time + '] ' + esc(l.m) + '</div>';
    }).join('') + '</div>';
  }
  box.className = 'dbg' + (DEBUG.open ? ' open' : '');
  box.innerHTML = html;
}
function dbgToggle() { DEBUG.open = !DEBUG.open; renderDebug(); }
function dbgClear() { DEBUG.logs = []; renderDebug(); }
window.addEventListener('error', function (e) {
  var msg = (e.message) || (e.error && e.error.message) || '未知脚本错误';
  if (e.filename) msg += ' @ ' + e.filename + ':' + e.lineno;
  dbg('JS错误: ' + msg, true);
});
window.addEventListener('unhandledrejection', function (e) {
  var r = e.reason;
  dbg('Promise异常: ' + (r && r.message ? r.message : String(r)), true);
});

function esc(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}
function humanSize(b) {
  var u = ['B', 'KB', 'MB', 'GB', 'TB'];
  var s = b, i = 0;
  while (s >= 1024 && i < u.length - 1) { s /= 1024; i++; }
  return i === 0 ? b + ' ' + u[0] : (s.toFixed(1) + ' ' + u[i]);
}
function renderItem(it) {
  if (it.kind === 'file') {
    return '<div class="entry"><span class="ico"><svg class="ico-svg"><use href="#doc-icon"/></svg></span>'
      + '<span class="name">' + esc(it.name) + '</span>'
      + '<span class="size">' + humanSize(it.size) + '</span>'
      + '<a class="btn" href="/d/' + esc(it.id) + '"><svg class="btn-svg"><use href="#dl-icon"/></svg><span>下载</span></a></div>';
  }
  return '<div class="entry"><span class="ico"><svg class="ico-svg"><use href="#text-icon"/></svg></span>'
    + '<span class="name">' + esc(it.name) + '</span>'
    + '<span class="size">' + humanSize(it.size) + '</span>'
    + '<button class="btn" type="button" onclick="copyText(\'t-' + esc(it.id) + '\', this)"><svg class="btn-svg"><use href="#cp-icon"/></svg><span>复制</span></button></div>'
    + '<pre id="t-' + esc(it.id) + '" class="text collapsed">' + esc(it.text || '') + '</pre>'
    + '<button class="toggle" type="button" onclick="toggleText(this)">展开</button>';
}
function getStates() {
  var s = {};
  document.querySelectorAll('.text').forEach(function (pre) {
    var id = pre.id.replace(/^t-/, '');
    s[id] = !pre.classList.contains('collapsed');
  });
  return s;
}
function restoreStates(s) {
  if (!s) return;
  Object.keys(s).forEach(function (id) {
    var pre = document.getElementById('t-' + id);
    if (!pre) return;
    var expanded = s[id];
    var btn = pre.nextElementSibling;
    if (expanded) { pre.classList.remove('collapsed'); if (btn && btn.classList.contains('toggle')) btn.textContent = '折叠'; }
    else { pre.classList.add('collapsed'); if (btn && btn.classList.contains('toggle')) btn.textContent = '展开'; }
  });
}
function refresh() {
  var states = getStates();
  dbg('刷新列表...');
  fetch('/api/items').then(function (r) {
    if (!r.ok) throw new Error('列表 HTTP ' + r.status);
    return r.json();
  }).then(function (items) {
    dbg('列表已获取: ' + (items ? items.length : 0) + ' 项');
    var list = document.getElementById('list');
    if (!items || !items.length) { list.innerHTML = '<div class="empty">还没有共享内容，在 jPaste 面板中添加文件或文本。</div>'; }
    else { list.innerHTML = items.map(renderItem).join(''); }
    restoreStates(states);
    initToggles();
  }).catch(function (err) {
    dbg('刷新失败: ' + (err && err.message ? err.message : err), true);
  });
}
function initToggles() {
  var pres = document.querySelectorAll('.text.collapsed');
  pres.forEach(function (pre) {
    if (pre.scrollHeight <= pre.clientHeight + 2) {
      pre.classList.remove('collapsed');
      var btn = pre.nextElementSibling;
      if (btn && btn.classList.contains('toggle')) btn.style.display = 'none';
    }
  });
}
function toggleText(btn) {
  var pre = btn.previousElementSibling;
  if (pre.classList.contains('collapsed')) { pre.classList.remove('collapsed'); btn.textContent = '折叠'; }
  else { pre.classList.add('collapsed'); btn.textContent = '展开'; }
}
function copyText(id, btn) {
  var el = document.getElementById(id);
  if (!el) return;
  var text = el.textContent;
  function done(ok) {
    if (!btn) return;
    var span = btn.querySelector('span');
    if (ok && span) { span.textContent = '已复制'; setTimeout(function () { span.textContent = '复制'; }, 1200); }
  }
  function fallback() {
    var ta = document.createElement('textarea');
    ta.value = text; ta.style.position = 'fixed'; ta.style.top = '-9999px'; ta.style.opacity = '0';
    document.body.appendChild(ta); ta.focus(); ta.select();
    var ok = false; try { ok = document.execCommand('copy'); } catch (e) { ok = false; }
    document.body.removeChild(ta); done(ok);
  }
  if (navigator.clipboard && window.isSecureContext) {
    navigator.clipboard.writeText(text).then(function () { done(true); }, function () { fallback(); });
  } else { fallback(); }
}
window.addEventListener('DOMContentLoaded', function () {
  renderDebug();
  dbg('页面已加载 (UA: ' + navigator.userAgent + ')');
  refresh();
  setInterval(refresh, 5000);
});
