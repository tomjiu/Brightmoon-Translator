use super::{OverlayContent, OverlayLevel};

/// Build overlay HTML based on the display level
pub fn build_html(content: &OverlayContent, level: OverlayLevel, dismiss_ms: u64) -> String {
    match level {
        OverlayLevel::Minimal => build_l1_html(&content.translated, dismiss_ms),
        OverlayLevel::Standard => build_l2_html(&content.source, &content.translated),
        OverlayLevel::Full => build_l3_html(&content.source, &content.translated),
    }
}

/// Build a shell HTML document that can be loaded once via HTTP and then
/// updated via eval() for content changes. This avoids re-encoding the
/// entire HTML as a data URI on every update.
pub fn build_shell_html() -> String {
    r##"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
body { background: transparent; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; overflow: hidden; }
.overlay-container { width: 100vw; height: 100vh; display: flex; align-items: flex-start; justify-content: flex-start; }
.card { background: rgba(26, 27, 38, 0.95); border: 1px solid rgba(59, 66, 97, 0.8); border-radius: 10px; padding: 10px 14px; color: #c0caf5; font-size: 13px; line-height: 1.5; user-select: text; box-shadow: 0 6px 24px rgba(0, 0, 0, 0.35); max-width: 400px; opacity: 0; transform: translateY(-4px); transition: opacity 0.15s ease-out, transform 0.15s ease-out; }
.card.visible { opacity: 1; transform: translateY(0); }
.card.level-1 { background: rgba(26, 27, 38, 0.92); border-color: rgba(59, 66, 97, 0.6); border-radius: 8px; box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3); max-width: none; }
.card.level-3 { padding: 12px 16px; font-size: 14px; line-height: 1.6; pointer-events: auto; box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4); width: 100vw; height: 100vh; max-width: none; overflow: auto; border-radius: 10px; }
.header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; padding-bottom: 8px; border-bottom: 1px solid rgba(59, 66, 97, 0.5); }
.title { font-size: 11px; color: #7aa2f7; font-weight: 600; text-transform: uppercase; letter-spacing: 0.5px; }
.actions { display: flex; gap: 4px; }
.btn { background: rgba(59, 66, 97, 0.5); border: 1px solid rgba(59, 66, 97, 0.8); color: #a9b1d6; border-radius: 6px; padding: 3px 10px; font-size: 11px; cursor: pointer; transition: all 0.15s ease; }
.btn:hover { background: rgba(122, 162, 247, 0.2); border-color: #7aa2f7; color: #c0caf5; }
.btn-close:hover { background: rgba(247, 118, 142, 0.2); border-color: #f7768e; color: #f7768e; }
.btn-copy.done { background: rgba(158, 206, 106, 0.2); border-color: #9ece6a; color: #9ece6a; }
.btn-pin.active { background: rgba(249, 226, 175, 0.2); border-color: #f9e2af; color: #f9e2af; }
.btn-passthrough.active { background: rgba(137, 180, 250, 0.2); border-color: #89b4fa; color: #89b4fa; }
.source { color: #565f89; font-size: 12px; margin-bottom: 6px; max-height: 60px; overflow: hidden; text-overflow: ellipsis; }
.source.has-border { margin-bottom: 8px; padding-bottom: 8px; max-height: none; border-bottom: 1px solid rgba(59, 66, 97, 0.3); }
.translated { color: #c0caf5; white-space: pre-wrap; word-break: break-word; }
.source:empty { display: none; }
.actions-row { display: flex; gap: 4px; margin-top: 8px; justify-content: flex-end; }
.hidden { display: none !important; }
.content-update { animation: contentFade 0.12s ease-out; }
@keyframes contentFade { from { opacity: 0.6; } to { opacity: 1; } }
</style>
</head>
<body>
<div class="overlay-container">
  <div class="card" id="overlayCard">
    <div class="header hidden" id="overlayHeader">
      <span class="title">Translation</span>
      <div class="actions">
        <button class="btn btn-pin" id="pinBtn" title="Pin">&#128204;</button>
        <button class="btn btn-passthrough" id="passthroughBtn" title="Click Through">&#128070;</button>
        <button class="btn btn-copy" id="headerCopyBtn">Copy</button>
        <button class="btn btn-close" id="headerCloseBtn">Close</button>
      </div>
    </div>
    <div class="source" id="sourceEl" data-role="source"></div>
    <div class="translated" id="translatedEl" data-role="translated"></div>
    <div class="actions-row hidden" id="l2Actions">
      <button class="btn" id="copyBtn">Copy</button>
      <button class="btn" id="closeBtn">Close</button>
    </div>
  </div>
</div>
<script>
(function() {
  'use strict';
  var currentLevel = 0;
  var dismissTimer = null;
  var pendingUpdate = null;
  function scheduleUpdate(source, translated, level) {
    if (pendingUpdate) cancelAnimationFrame(pendingUpdate);
    pendingUpdate = requestAnimationFrame(function() { applyUpdate(source, translated, level); pendingUpdate = null; });
  }
  function applyUpdate(source, translated, level) {
    var card = document.getElementById('overlayCard');
    var sourceEl = document.getElementById('sourceEl');
    var translatedEl = document.getElementById('translatedEl');
    var header = document.getElementById('overlayHeader');
    var l2Actions = document.getElementById('l2Actions');
    if (!card || !sourceEl || !translatedEl) return;
    card.classList.remove('level-1', 'level-2', 'level-3');
    if (level === 1) { card.classList.add('level-1'); header.classList.add('hidden'); l2Actions.classList.add('hidden'); sourceEl.classList.remove('has-border'); }
    else if (level === 2) { header.classList.add('hidden'); l2Actions.classList.remove('hidden'); sourceEl.classList.remove('has-border'); }
    else { card.classList.add('level-3'); header.classList.remove('hidden'); l2Actions.classList.add('hidden'); sourceEl.classList.add('has-border'); }
    if (sourceEl.textContent !== source) sourceEl.textContent = source;
    if (translatedEl.textContent !== translated) translatedEl.textContent = translated;
    card.classList.add('content-update');
    setTimeout(function() { card.classList.remove('content-update'); }, 150);
    card.classList.add('visible');
    currentLevel = level;
  }
  function invoke(cmd, args) { if (window.__TAURI__ && window.__TAURI__.core) return window.__TAURI__.core.invoke(cmd, args); return Promise.reject('Tauri not available'); }
  function setupCopyBtn(btn) { if (!btn) return; btn.onclick = async function() { var trans = document.getElementById('translatedEl').textContent; try { await navigator.clipboard.writeText(trans); btn.textContent = 'Copied!'; btn.classList.add('done'); setTimeout(function() { btn.textContent = 'Copy'; btn.classList.remove('done'); }, 1500); } catch(e) { var ta = document.createElement('textarea'); ta.value = trans; document.body.appendChild(ta); ta.select(); document.execCommand('copy'); document.body.removeChild(ta); btn.textContent = 'Copied!'; btn.classList.add('done'); setTimeout(function() { btn.textContent = 'Copy'; btn.classList.remove('done'); }, 1500); } }; }
  function setupCloseBtn(btn) { if (!btn) return; btn.onclick = function() { invoke('close_overlay'); }; }
  var pinBtn = document.getElementById('pinBtn');
  if (pinBtn) { pinBtn.classList.add('active'); pinBtn.onclick = async function() { try { var pinned = await invoke('pin_overlay'); if (pinned) pinBtn.classList.add('active'); else pinBtn.classList.remove('active'); } catch(e) {} }; }
  var passthroughBtn = document.getElementById('passthroughBtn');
  if (passthroughBtn) { passthroughBtn.onclick = async function() { var active = !passthroughBtn.classList.contains('active'); try { await invoke('set_overlay_click_through', { ignore: active }); if (active) passthroughBtn.classList.add('active'); else passthroughBtn.classList.remove('active'); } catch(e) {} }; }
  if (window.__TAURI__ && window.__TAURI__.event) { window.__TAURI__.event.listen('overlay-click-through-off', function() { if (passthroughBtn) passthroughBtn.classList.remove('active'); }); }
  setupCopyBtn(document.getElementById('copyBtn'));
  setupCopyBtn(document.getElementById('headerCopyBtn'));
  setupCloseBtn(document.getElementById('closeBtn'));
  setupCloseBtn(document.getElementById('headerCloseBtn'));
  document.addEventListener('keydown', function(e) { if (e.key === 'Escape') invoke('close_overlay'); });
  document.addEventListener('click', function(e) { if (currentLevel === 1) invoke('close_overlay'); });
  window.__overlayUpdate = function(source, translated, level, dismissMs) {
    if (dismissTimer) { clearTimeout(dismissTimer); dismissTimer = null; }
    scheduleUpdate(source, translated, level || 2);
    if (level === 1 && dismissMs > 0) { dismissTimer = setTimeout(function() { invoke('close_overlay'); }, dismissMs); }
  };
  window.__overlayUpdate('', '', 2, 0);
})();
</script>
</body>
</html>"##.to_string()
}

/// Build a JS script that calls the shell's __overlayUpdate function.
/// Escapes all special characters to prevent XSS injection via script context.
pub fn build_update_script(source: &str, translated: &str, level: OverlayLevel, dismiss_ms: u64) -> String {
    // Escape for JavaScript string literal context (single-quoted)
    // Must handle: backslash, single quote, newlines, carriage returns,
    // HTML close tags (</script>), null bytes, and Unicode line/paragraph separators
    let escape_js = |s: &str| -> String {
        s.replace('\\', "\\\\")
            .replace('\'', "\\'")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
            .replace('\0', "\\0")
            .replace("</", "<\\/")
            .replace('\u{2028}', "\\u2028") // Unicode Line Separator
            .replace('\u{2029}', "\\u2029") // Unicode Paragraph Separator
    };
    let src_escaped = escape_js(source);
    let trans_escaped = escape_js(translated);
    let level_num = level as u8;
    format!("window.__overlayUpdate('{}', '{}', {}, {});", src_escaped, trans_escaped, level_num, dismiss_ms)
}

/// L1: Minimal overlay - just translated text, auto-dismiss after dismiss_ms
fn build_l1_html(translated: &str, dismiss_ms: u64) -> String {
    let escaped = html_escape::encode_text(translated);
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{ background: transparent; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; overflow: hidden; }}
.card {{
  background: rgba(26, 27, 38, 0.92);
  border: 1px solid rgba(59, 66, 97, 0.6);
  border-radius: 8px;
  padding: 10px 14px;
  color: #c0caf5;
  font-size: 13px;
  line-height: 1.5;
  user-select: text;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
  animation: fadeIn 0.15s ease-out;
}}
@keyframes fadeIn {{ from {{ opacity: 0; transform: translateY(-4px); }} to {{ opacity: 1; transform: translateY(0); }} }}
</style>
</head>
<body>
<div class="card">{escaped}</div>
<script>
setTimeout(() => window.__TAURI__?.core.invoke('close_overlay'), {dismiss_ms});
document.addEventListener('click', () => window.__TAURI__?.core.invoke('close_overlay'));
document.addEventListener('keydown', e => {{ if (e.key === 'Escape') window.__TAURI__?.core.invoke('close_overlay'); }});
</script>
</body>
</html>"#
    )
}

/// L2: Standard overlay - source + translated, copy button
fn build_l2_html(source: &str, translated: &str) -> String {
    let src_escaped = html_escape::encode_text(source);
    let trans_escaped = html_escape::encode_text(translated);
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{ background: transparent; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; overflow: hidden; }}
.card {{
  background: rgba(26, 27, 38, 0.95);
  border: 1px solid rgba(59, 66, 97, 0.8);
  border-radius: 10px;
  padding: 10px 14px;
  color: #c0caf5;
  font-size: 13px;
  line-height: 1.5;
  user-select: text;
  box-shadow: 0 6px 24px rgba(0, 0, 0, 0.35);
  animation: fadeIn 0.15s ease-out;
  max-width: 400px;
}}
.source {{ color: #565f89; font-size: 12px; margin-bottom: 6px; max-height: 60px; overflow: hidden; text-overflow: ellipsis; }}
.translated {{ color: #c0caf5; }}
.actions {{ display: flex; gap: 4px; margin-top: 8px; justify-content: flex-end; }}
.btn {{
  background: rgba(59, 66, 97, 0.5);
  border: 1px solid rgba(59, 66, 97, 0.8);
  color: #a9b1d6;
  border-radius: 6px;
  padding: 3px 10px;
  font-size: 11px;
  cursor: pointer;
}}
.btn:hover {{ background: rgba(122, 162, 247, 0.2); border-color: #7aa2f7; }}
.btn.done {{ background: rgba(158, 206, 106, 0.2); border-color: #9ece6a; color: #9ece6a; }}
@keyframes fadeIn {{ from {{ opacity: 0; transform: translateY(-4px); }} to {{ opacity: 1; transform: translateY(0); }} }}
</style>
</head>
<body>
<div class="card">
  <div class="source">{src_escaped}</div>
  <div class="translated">{trans_escaped}</div>
  <div class="actions">
    <button class="btn" id="copyBtn">Copy</button>
    <button class="btn" id="closeBtn">Close</button>
  </div>
</div>
<script>
const trans = document.querySelector('.translated').textContent;
document.getElementById('copyBtn').onclick = async () => {{
  await navigator.clipboard.writeText(trans);
  const btn = document.getElementById('copyBtn');
  btn.textContent = 'Copied!'; btn.classList.add('done');
  setTimeout(() => {{ btn.textContent = 'Copy'; btn.classList.remove('done'); }}, 1500);
}};
document.getElementById('closeBtn').onclick = () => window.__TAURI__?.core.invoke('close_overlay');
document.addEventListener('keydown', e => {{ if (e.key === 'Escape') window.__TAURI__?.core.invoke('close_overlay'); }});
</script>
</body>
</html>"#
    )
}

/// L3: Full overlay - source + translated, all controls
fn build_l3_html(source: &str, translated: &str) -> String {
    let src_escaped = html_escape::encode_text(source);
    let trans_escaped = html_escape::encode_text(translated);
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{ background: transparent; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; overflow: hidden; }}
.card {{
  background: rgba(26, 27, 38, 0.95);
  border: 1px solid rgba(59, 66, 97, 0.8);
  border-radius: 10px;
  padding: 12px 16px;
  color: #c0caf5;
  font-size: 14px;
  line-height: 1.6;
  user-select: text;
  pointer-events: auto;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  animation: fadeIn 0.15s ease-out;
  width: 100vw;
  height: 100vh;
  overflow: auto;
}}
.header {{
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
  padding-bottom: 8px;
  border-bottom: 1px solid rgba(59, 66, 97, 0.5);
}}
.title {{ font-size: 11px; color: #7aa2f7; font-weight: 600; text-transform: uppercase; letter-spacing: 0.5px; }}
.actions {{ display: flex; gap: 4px; }}
.btn {{
  background: rgba(59, 66, 97, 0.5);
  border: 1px solid rgba(59, 66, 97, 0.8);
  color: #a9b1d6;
  border-radius: 6px;
  padding: 4px 10px;
  font-size: 11px;
  cursor: pointer;
  transition: all 0.15s ease;
}}
.btn:hover {{ background: rgba(122, 162, 247, 0.2); border-color: #7aa2f7; color: #c0caf5; }}
.btn-close:hover {{ background: rgba(247, 118, 142, 0.2); border-color: #f7768e; color: #f7768e; }}
.btn-copy.done {{ background: rgba(158, 206, 106, 0.2); border-color: #9ece6a; color: #9ece6a; }}
.btn-pin.active {{ background: rgba(249, 226, 175, 0.2); border-color: #f9e2af; color: #f9e2af; }}
.btn-passthrough.active {{ background: rgba(137, 180, 250, 0.2); border-color: #89b4fa; color: #89b4fa; }}
.source {{ color: #565f89; font-size: 12px; margin-bottom: 8px; padding-bottom: 8px; border-bottom: 1px solid rgba(59, 66, 97, 0.3); }}
.translated {{ white-space: pre-wrap; word-break: break-word; }}
@keyframes fadeIn {{ from {{ opacity: 0; transform: translateY(-4px); }} to {{ opacity: 1; transform: translateY(0); }} }}
</style>
</head>
<body>
<div class="card">
  <div class="header">
    <span class="title">Translation</span>
    <div class="actions">
      <button class="btn btn-pin" id="pinBtn" title="Pin">📌</button>
      <button class="btn btn-passthrough" id="passthroughBtn" title="Click Through">👆</button>
      <button class="btn btn-copy" id="copyBtn">Copy</button>
      <button class="btn btn-close" id="closeBtn">Close</button>
    </div>
  </div>
  <div class="source" data-role="source">{src_escaped}</div>
  <div class="translated" data-role="translated">{trans_escaped}</div>
</div>
<script>
const trans = document.querySelector('.translated').textContent;
document.getElementById('copyBtn').onclick = async () => {{
  await navigator.clipboard.writeText(trans);
  const btn = document.getElementById('copyBtn');
  btn.textContent = 'Copied!'; btn.classList.add('done');
  setTimeout(() => {{ btn.textContent = 'Copy'; btn.classList.remove('done'); }}, 1500);
}};
const pinBtn = document.getElementById('pinBtn');
pinBtn.classList.add('active'); // starts pinned
pinBtn.onclick = async () => {{
  const pinned = await window.__TAURI__?.core.invoke('pin_overlay');
  if (pinned) {{ pinBtn.classList.add('active'); }}
  else {{ pinBtn.classList.remove('active'); }}
}};
const passthroughBtn = document.getElementById('passthroughBtn');
passthroughBtn.onclick = async () => {{
  const active = !passthroughBtn.classList.contains('active');
  await window.__TAURI__?.core.invoke('set_overlay_click_through', {{ ignore: active }});
  if (active) {{ passthroughBtn.classList.add('active'); }}
  else {{ passthroughBtn.classList.remove('active'); }}
}};
// Listen for click-through disabled event from global shortcut
window.__TAURI__?.event.listen('overlay-click-through-off', () => {{
  passthroughBtn.classList.remove('active');
}});
document.getElementById('closeBtn').onclick = () => window.__TAURI__?.core.invoke('close_overlay');
document.addEventListener('keydown', e => {{ if (e.key === 'Escape') window.__TAURI__?.core.invoke('close_overlay'); }});
</script>
</body>
</html>"#
    )
}
