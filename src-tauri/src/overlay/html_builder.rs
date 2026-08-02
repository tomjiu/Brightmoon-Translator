use super::{OverlayContent, OverlayLevel};

/// Build overlay HTML — single solid card, fills window (no black chrome + white inset).
///
/// `pin_label`: when Some, the HTML includes a resize event listener that
/// reports the new window size back to the backend via the
/// `update_pinned_card_size` Tauri command. Pass None for transient overlays.
pub fn build_html(
    content: &OverlayContent,
    level: OverlayLevel,
    dismiss_ms: u64,
    pin_label: Option<&str>,
) -> String {
    match level {
        OverlayLevel::Minimal => {
            build_card_html(None, &content.translated, dismiss_ms.max(2000), true, pin_label)
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
                pin_label,
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
  --bg: #fafbfc;
  --bg-elev: #ffffff;
  --fg: #0f172a;
  --muted: #64748b;
  --border: rgba(15, 23, 42, 0.10);
  --shadow: 0 10px 30px rgba(15, 23, 42, 0.12), 0 2px 8px rgba(15, 23, 42, 0.06);
  --accent: #2563eb;
}
"#
        .to_string()
    } else {
        r#"
:root {
  --bg: #14151a;
  --bg-elev: #1c1e26;
  --fg: #f1f5f9;
  --muted: #94a3b8;
  --border: rgba(255, 255, 255, 0.10);
  --shadow: 0 12px 36px rgba(0, 0, 0, 0.55), 0 2px 8px rgba(0, 0, 0, 0.35);
  --accent: #60a5fa;
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
  background: var(--bg-elev); color: var(--fg);
  border: 1px solid var(--border); border-radius: 12px;
  box-shadow: var(--shadow);
  padding: 11px 13px; font-size: 13px; line-height: 1.5;
}}
.source {{
  color: var(--muted); font-size: 12px; margin-bottom: 6px;
  padding-bottom: 6px; border-bottom: 1px solid var(--border);
}}
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

/// Structured dictionary card (preferred; no body re-parse).
pub fn build_dict_card_structured(
    card: &crate::selection::present::DictCard,
    dismiss_ms: u64,
) -> String {
    let mut defs_html = String::new();
    for m in &card.meanings {
        for d in &m.defs {
            if m.pos.is_empty() || m.pos.eq_ignore_ascii_case("ecdict") {
                defs_html.push_str(&format!(
                    r#"<div class="def">{}</div>"#,
                    html_escape::encode_text(d)
                ));
            } else {
                defs_html.push_str(&format!(
                    r#"<div class="def"><span class="pos">{}</span> {}</div>"#,
                    html_escape::encode_text(&m.pos),
                    html_escape::encode_text(d)
                ));
            }
        }
    }
    let phon_html = card
        .phonetic
        .as_ref()
        .map(|p| {
            format!(
                r#"<span class="phon">{}</span>"#,
                html_escape::encode_text(p)
            )
        })
        .unwrap_or_default();
    render_dict_card_shell(&card.word, &phon_html, &defs_html, dismiss_ms)
}

fn render_dict_card_shell(word: &str, phon_html: &str, defs_html: &str, dismiss_ms: u64) -> String {
    let dismiss = if dismiss_ms > 0 {
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
  width: 100%; height: 100%;
  background: var(--bg-elev); color: var(--fg);
  border: 1px solid var(--border); border-radius: 12px;
  box-shadow: var(--shadow);
  padding: 11px 13px; font-size: 13px; line-height: 1.5;
  user-select: text;
}}
.badge {{
  display:inline-block; font-size:10px; font-weight:600;
  color: var(--accent); border: 1px solid var(--accent);
  border-radius: 4px; padding: 0 5px; margin-bottom: 6px; opacity: 0.9;
}}
.head {{ font-size: 15px; font-weight: 600; margin-bottom: 4px; }}
.phon {{ color: var(--muted); font-weight: 400; font-size: 12px; margin-left: 6px; }}
.pos {{
  display:inline-block; font-size:10px; color: var(--bg);
  background: var(--accent); border-radius: 3px; padding: 0 4px;
  margin-right: 4px; vertical-align: middle;
}}
.def {{ margin-top: 4px; word-break: break-word; }}
</style>
</head>
<body>
<div class="card">
  <div class="badge">词典</div>
  <div class="head">{word}{phon}</div>
  {defs}
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
        word = html_escape::encode_text(word),
        phon = phon_html,
        defs = defs_html,
        dismiss = dismiss,
    )
}

fn build_card_html(
    source: Option<&str>,
    translated: &str,
    dismiss_ms: u64,
    auto: bool,
    pin_label: Option<&str>,
) -> String {
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
    // Tier4-3: resize listener for pinned cards. Reports new window size
    // to the backend so PinSlot metadata stays in sync. Debounced 200ms
    // to avoid flooding the backend during drag.
    let resize_script = if let Some(label) = pin_label {
        format!(
            r#"(function(){{
  var t=null;
  function sendSize(){{
    t=null;
    var w=window.innerWidth||document.documentElement.clientWidth;
    var h=window.innerHeight||document.documentElement.clientHeight;
    if(window.__TAURI__&&window.__TAURI__.core){{
      window.__TAURI__.core.invoke('update_pinned_card_size',{{label:{lbl},width:w,height:h}});
    }}
  }}
  window.addEventListener('resize',function(){{
    if(t)clearTimeout(t);
    t=setTimeout(sendSize,200);
  }});
}})();"#,
            lbl = serde_json::json!(label)
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
  background: var(--bg-elev);
  color: var(--fg);
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: var(--shadow);
  padding: 11px 13px;
  font-size: 13px;
  line-height: 1.5;
  user-select: text;
}}
.source {{
  color: var(--muted);
  font-size: 12px;
  margin-bottom: 6px;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--border);
  max-height: 2.8em;
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
{resize}
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
        resize = resize_script,
    )
}
