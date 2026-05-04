use super::{OverlayContent, OverlayLevel};

/// Build overlay HTML based on the display level
pub fn build_html(content: &OverlayContent, level: OverlayLevel, dismiss_ms: u64) -> String {
    match level {
        OverlayLevel::Minimal => build_l1_html(&content.translated, dismiss_ms),
        OverlayLevel::Standard => build_l2_html(&content.source, &content.translated),
        OverlayLevel::Full => build_l3_html(&content.source, &content.translated),
    }
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
  max-width: 450px;
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
  <div class="source">{src_escaped}</div>
  <div class="translated">{trans_escaped}</div>
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
