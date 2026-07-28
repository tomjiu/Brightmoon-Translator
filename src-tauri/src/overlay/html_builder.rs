use super::{OverlayContent, OverlayLevel};

/// Build overlay HTML — single solid card, fills window (no black chrome + white inset).
pub fn build_html(content: &OverlayContent, level: OverlayLevel, dismiss_ms: u64) -> String {
    match level {
        OverlayLevel::Minimal => {
            build_card_html(None, &content.translated, dismiss_ms.max(2000), true)
        },
        OverlayLevel::Standard | OverlayLevel::Full => {
            let src = content.source.trim();
            let show_src = !src.is_empty()
                && src != content.translated.trim()
                && !content.translated.trim().starts_with(src);
            build_card_html(
                if show_src { Some(src) } else { None },
                &content.translated,
                dismiss_ms.max(3500),
                true,
            )
        },
    }
}

/// Theme from FE (localStorage) via set_overlay_theme; default dark to match app.
fn theme_css() -> String {
    let light = crate::overlay::window_manager::overlay_theme_is_light();
    if light {
        r#"
:root {
  --bg: #ffffff;
  --fg: #111827;
  --muted: #6b7280;
  --border: rgba(0,0,0,0.12);
}
"#
        .to_string()
    } else {
        r#"
:root {
  --bg: #1a1a1e;
  --fg: #f3f4f6;
  --muted: #9ca3af;
  --border: rgba(255,255,255,0.12);
}
"#
        .to_string()
    }
}

pub fn build_shell_html() -> String {
    format!(
        r##"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
* {{ margin:0; padding:0; box-sizing:border-box; }}
{theme}
html,body {{
  width:100%; height:100%; margin:0;
  background: var(--bg) !important;
  font-family: "Segoe UI","Microsoft YaHei",sans-serif;
  overflow:hidden;
}}
.card {{
  width:100%; height:100%;
  background: var(--bg); color: var(--fg);
  border: 1px solid var(--border); border-radius: 10px;
  padding: 10px 12px; font-size: 13px; line-height: 1.45;
}}
.source {{ color: var(--muted); font-size: 12px; margin-bottom: 4px; }}
.translated {{ color: var(--fg); white-space: pre-wrap; word-break: break-word; }}
</style>
</head>
<body>
<div class="card">
  <div class="source" id="sourceEl" style="display:none"></div>
  <div class="translated" id="translatedEl"></div>
</div>
<script>
(function(){{
  var t=null;
  window.__overlayUpdate=function(source,translated,level,dismissMs){{
    if(t){{clearTimeout(t);t=null;}}
    var s=document.getElementById('sourceEl'), d=document.getElementById('translatedEl');
    if(s){{ s.textContent=source||''; s.style.display=source?'block':'none'; }}
    if(d) d.textContent=translated||'';
    if(dismissMs>0) t=setTimeout(function(){{
      window.__TAURI__&&window.__TAURI__.core&&window.__TAURI__.core.invoke('close_overlay');
    }},dismissMs);
  }};
}})();
</script>
</body>
</html>"##,
        theme = theme_css()
    )
}

pub fn build_update_script(
    source: &str,
    translated: &str,
    level: OverlayLevel,
    dismiss_ms: u64,
) -> String {
    let escape_js = |s: &str| -> String {
        s.replace('\\', "\\\\")
            .replace('\'', "\\'")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
            .replace('\0', "\\0")
            .replace("</", "<\\/")
            .replace('\u{2028}', "\\u2028")
            .replace('\u{2029}', "\\u2029")
    };
    format!(
        "window.__overlayUpdate('{}', '{}', {}, {});",
        escape_js(source),
        escape_js(translated),
        level as u8,
        dismiss_ms
    )
}

fn build_card_html(source: Option<&str>, translated: &str, dismiss_ms: u64, auto: bool) -> String {
    let body = html_escape::encode_text(translated);
    let src_html = source
        .map(|s| {
            format!(
                r#"<div class="source">{}</div>"#,
                html_escape::encode_text(s)
            )
        })
        .unwrap_or_default();
    let dismiss = if auto && dismiss_ms > 0 {
        format!(
            "setTimeout(function(){{window.__TAURI__&&window.__TAURI__.core&&window.__TAURI__.core.invoke('close_overlay');}},{});",
            dismiss_ms
        )
    } else {
        String::new()
    };
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
* {{ margin:0; padding:0; box-sizing:border-box; }}
{theme}
html, body {{
  width:100%; height:100%; margin:0;
  background: var(--bg) !important;
  font-family: "Segoe UI","Microsoft YaHei",sans-serif;
  overflow: hidden;
}}
.card {{
  width: 100%;
  height: 100%;
  background: var(--bg);
  color: var(--fg);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 10px 12px;
  font-size: 13px;
  line-height: 1.45;
  user-select: text;
}}
.source {{
  color: var(--muted);
  font-size: 12px;
  margin-bottom: 4px;
  max-height: 2.6em;
  overflow: hidden;
}}
.translated {{
  color: var(--fg);
  white-space: pre-wrap;
  word-break: break-word;
}}
</style>
</head>
<body>
<div class="card">
  {src}
  <div class="translated">{body}</div>
</div>
<script>
{dismiss}
document.addEventListener('keydown', function(e) {{
  if (e.key === 'Escape') window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke('close_overlay');
}});
</script>
</body>
</html>"#,
        theme = theme_css(),
        src = src_html,
        body = body,
        dismiss = dismiss,
    )
}
