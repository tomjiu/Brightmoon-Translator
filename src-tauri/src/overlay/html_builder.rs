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

/// Theme from FE (localStorage) via `set_overlay_theme`; default dark to match app.
fn theme_css() -> String {
    let light = crate::overlay::window_manager::overlay_theme_is_light();
    if light {
        r"
:root {
  --bg: #fafbfc;
  --bg-elev: #ffffff;
  --fg: #0f172a;
  --muted: #64748b;
  --border: rgba(15, 23, 42, 0.10);
  --shadow: 0 10px 30px rgba(15, 23, 42, 0.12), 0 2px 8px rgba(15, 23, 42, 0.06);
  --accent: #2563eb;
}
"
        .to_string()
    } else {
        r"
:root {
  --bg: #14151a;
  --bg-elev: #1c1e26;
  --fg: #f1f5f9;
  --muted: #94a3b8;
  --border: rgba(255, 255, 255, 0.10);
  --shadow: 0 12px 36px rgba(0, 0, 0, 0.55), 0 2px 8px rgba(0, 0, 0, 0.35);
  --accent: #60a5fa;
}
"
        .to_string()
    }
}

pub fn build_shell_html() -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
* {{ margin:0; padding:0; box-sizing:border-box; }}
{theme}
html,body {{
  width:100%; min-height:100%; margin:0;
  background: var(--bg) !important;
  font-family: "Segoe UI","Microsoft YaHei",sans-serif;
}}
html::-webkit-scrollbar, body::-webkit-scrollbar {{ display:none; }}
.card {{
  width:100%; min-height:100%;
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
      if(window.__moonInvoke) window.__moonInvoke('close_overlay');
    }},dismissMs);
  }};
}})();
</script>
</body>
</html>"#,
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

/// Dismiss timer that clears any previously armed timer first.
///
/// P0 (Fix 2): the overlay window is reused via `document.documentElement.innerHTML`
/// replacement, which does NOT cancel pending `setTimeout` callbacks. Without a
/// shared handle, a new card got closed by the previous card's dismiss timer
/// ("card flash-close"). Storing the handle on `window` (which survives innerHTML
/// replacement) lets each new card's script clear the old timer before arming its own.
fn dismiss_script(dismiss_ms: u64) -> String {
    if dismiss_ms > 0 {
        format!(
            "if(window.__moonDismissTimer)clearTimeout(window.__moonDismissTimer);window.__moonDismissTimer=setTimeout(function(){{if(window.__moonInvoke)window.__moonInvoke('close_overlay');}},{dismiss_ms});"
        )
    } else {
        String::new()
    }
}

/// Escape handler bound exactly once per page lifetime.
///
/// Each innerHTML replacement re-runs the injected scripts, so a bare
/// `document.addEventListener('keydown', …)` would accumulate one listener per
/// card shown (memory + response lag over many hovers). Guarding with a global
/// flag keeps exactly one listener on `document`.
fn escape_script() -> &'static str {
    "if(!window.__moonEscBound){window.__moonEscBound=true;document.addEventListener('keydown',function(e){if(e.key==='Escape'&&window.__moonInvoke)window.__moonInvoke('close_overlay');});}"
}

/// Auto-fit: after the card renders, measure the actual content box and grow
/// the overlay window to fit — the Rust-side height estimate is heuristic and
/// long definitions / CJK wrapping can under-estimate, clipping the font. This
/// runs once per page (data: URL navigation replaces the document, so the
/// flag is per-page). Only grows (never shrinks the window below the estimate)
/// to avoid a visible resize jump.
///
/// Resolve the Tauri IPC invoke function. Overlay cards are raw data: URL HTML
/// (no bundler), so they cannot use the `@tauri-apps/api` ESM. Two globals may
/// expose invoke:
/// - `window.__TAURI__` — only when `app.withGlobalTauri` is enabled;
/// - `window.__TAURI_INTERNALS__.invoke` — always injected (this is the object
///   `@tauri-apps/api` wraps). Prefer the internals so fit works even if
///   withGlobalTauri is off / data: URL injection is skipped.
const INVOKE: &str = "window.__moonInvoke = (window.__TAURI__&&window.__TAURI__.core)?window.__TAURI__.core.invoke.bind(window.__TAURI__.core):(window.__TAURI_INTERNALS__&&window.__TAURI_INTERNALS__.invoke)?window.__TAURI_INTERNALS__.invoke.bind(window.__TAURI_INTERNALS__):null;";

fn fit_script() -> String {
    format!(
        r"(function(){{
  if(window.__moonFitDone)return; window.__moonFitDone=true;
  {INVOKE}
  function probe(){{
    var el=document.body;
    var w=el.scrollWidth, h=el.scrollHeight;
    var winW=window.innerWidth||document.documentElement.clientWidth;
    var winH=window.innerHeight||document.documentElement.clientHeight;
    var hasI = typeof window.__TAURI_INTERNALS__;
    var hasG = typeof window.__TAURI__;
    var inv = typeof window.__moonInvoke;
    var ta = typeof window.__TAURI__;
    try {{ document.title = 'DIAG|'+w+'|'+h+'|'+winW+'|'+winH+'|I:'+hasI+'|G:'+hasG+'|inv:'+inv+'|ta:'+ta; }} catch(e){{}}
  }}
  function fit(){{
    if(!window.__moonInvoke){{ document.title='DIAG-NOINVOKE'; return; }}
    var el=document.body;
    var w=el.scrollWidth, h=el.scrollHeight;
    var winW=window.innerWidth||document.documentElement.clientWidth;
    var winH=window.innerHeight||document.documentElement.clientHeight;
    var nw=Math.max(w,winW), nh=Math.max(h,winH);
    document.title='DIAG-FIT|'+nw+'|'+nh;
    window.__moonInvoke('resize_overlay',{{width:Math.min(Math.max(Math.ceil(nw),120),620),height:Math.min(Math.max(Math.ceil(nh),48),720)}});
  }}
  probe();
  if(document.readyState==='loading'){{document.addEventListener('DOMContentLoaded',function(){{probe();fit();}});}}
  else{{fit();}}
  window.addEventListener('load',function(){{setTimeout(function(){{probe();fit();}},10);}});
}})();"
    )
}

/// Structured dictionary card (preferred; no body re-parse).
/// P1: caps rendered defs at 8 so the card matches `dict_card_size`'s
/// estimate — previously 6 meanings × 3 defs could overflow the estimated
/// height and get truncated.
pub fn build_dict_card_structured(
    card: &crate::selection::present::DictCard,
    dismiss_ms: u64,
) -> String {
    let mut defs_html = String::new();
    let mut total = 0usize;
    for m in &card.meanings {
        for d in &m.defs {
            if total >= 8 {
                break;
            }
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
            total += 1;
        }
        if total >= 8 {
            break;
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

    // 🔊 audio button when we have an audio URL (WebView2 data: page can play https).
    let audio_html = card
        .audio_url
        .as_ref()
        .filter(|u| !u.trim().is_empty())
        .map(|u| {
            let js = u.replace('\\', "\\\\").replace('\'', "\\'");
            format!(
                r#"<button class="audio-btn" title="播放发音" onclick="var a=new Audio('{js}');a.play();">🔊</button>"#
            )
        })
        .unwrap_or_default();

    // 例句 section.
    let examples_html = if card.examples.is_empty() {
        String::new()
    } else {
        let mut rows = String::new();
        for e in card.examples.iter().take(3) {
            if e.en.trim().is_empty() {
                continue;
            }
            rows.push_str(&format!(
                r#"<div class="ex"><div class="ex-en">{}</div>{}</div>"#,
                html_escape::encode_text(&e.en),
                if e.zh.trim().is_empty() {
                    String::new()
                } else {
                    format!(
                        r#"<div class="ex-zh">{}</div>"#,
                        html_escape::encode_text(&e.zh)
                    )
                }
            ));
        }
        if rows.is_empty() {
            String::new()
        } else {
            format!(r#"<div class="sec">例句</div>{rows}"#)
        }
    };

    // Collins section.
    let collins_html = if card.collins.is_empty() {
        String::new()
    } else {
        let mut rows = String::new();
        for c in card.collins.iter().take(2) {
            rows.push_str(&format!(
                r#"<div class="collins"><span class="cpos">{}</span> {}</div>"#,
                html_escape::encode_text(
                    if c.pos_cn.trim().is_empty() {
                        c.pos.as_str()
                    } else {
                        c.pos_cn.as_str()
                    }
                ),
                html_escape::encode_text(&c.english_def)
            ));
        }
        format!(r#"<div class="sec">柯林斯</div>{rows}"#)
    };

    // Source badges.
    let sources_html = if card.sources.is_empty() {
        String::new()
    } else {
        let badges: String = card
            .sources
            .iter()
            .take(6)
            .fold(String::new(), |mut out, s| {
                out.push_str(r#"<span class="src">"#);
                out.push_str(&html_escape::encode_text(s));
                out.push_str("</span>");
                out
            });
        format!(r#"<div class="srcs">{badges}</div>"#)
    };

    render_dict_card_shell(
        &card.word,
        &phon_html,
        &audio_html,
        &defs_html,
        &examples_html,
        &collins_html,
        &sources_html,
        dismiss_ms,
    )
}

fn render_dict_card_shell(
    word: &str,
    phon_html: &str,
    audio_html: &str,
    defs_html: &str,
    examples_html: &str,
    collins_html: &str,
    sources_html: &str,
    dismiss_ms: u64,
) -> String {
    let dismiss = dismiss_script(dismiss_ms);
    let escape = escape_script();
    let fit = fit_script();
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
* {{ margin:0; padding:0; box-sizing:border-box; }}
{theme}
html, body {{
  width:100%; min-height:100%; margin:0;
  background: var(--bg) !important;
  font-family: "Segoe UI","Microsoft YaHei",sans-serif;
}}
html::-webkit-scrollbar, body::-webkit-scrollbar {{ display:none; }}
.card {{
  width: 100%;
  min-height: 100%;
  background: var(--bg-elev); color: var(--fg);
  border: 1px solid var(--border); border-radius: 12px;
  box-shadow: var(--shadow);
  padding: 11px 13px; font-size: 13px; line-height: 1.5;
  user-select: text;
  display: flex; flex-direction: column;
}}
.badge {{
  display:inline-block; font-size:10px; font-weight:600;
  color: var(--accent); border: 1px solid var(--accent);
  border-radius: 4px; padding: 0 5px; margin-bottom: 6px; opacity: 0.9;
}}
.head {{
  display:flex; align-items:center; gap:6px;
  font-size: 16px; font-weight: 600; margin-bottom: 4px;
}}
.word {{ }}
.phon {{ color: var(--muted); font-weight: 400; font-size: 12px; }}
.audio-btn {{
  background: none; border: none; cursor: pointer;
  font-size: 13px; padding: 0 2px; opacity: 0.85;
}}
.audio-btn:hover {{ opacity: 1; }}
.defs {{ overflow: visible; }}
.pos {{
  display:inline-block; font-size:10px; color: var(--bg);
  background: var(--accent); border-radius: 3px; padding: 0 4px;
  margin-right: 4px; vertical-align: middle;
}}
.def {{ margin-top: 4px; word-break: break-word; }}
.sec {{
  margin-top: 8px; padding-top: 6px; border-top: 1px solid var(--border);
  font-size: 11px; color: var(--muted); font-weight: 600;
}}
.ex {{ margin-top: 5px; }}
.ex-en {{ font-weight: 500; word-break: break-word; }}
.ex-zh {{ color: var(--muted); font-size: 12px; word-break: break-word; }}
.collins {{ margin-top: 5px; word-break: break-word; }}
.cpos {{
  display:inline-block; font-size:10px; color: var(--accent);
  border: 1px solid var(--accent); border-radius: 3px; padding: 0 4px;
  margin-right: 4px; vertical-align: middle;
}}
.srcs {{
  margin-top: auto; padding-top: 6px;
  display: flex; gap: 4px; flex-wrap: wrap;
}}
.src {{
  font-size: 9px; color: var(--muted);
  border: 1px solid var(--border); border-radius: 3px; padding: 0 4px;
}}
</style>
</head>
<body>
<div class="card">
  <div class="badge">词典</div>
  <div class="head"><span class="word">{word}</span>{phon}{audio}</div>
  <div class="defs">{defs}</div>
  {examples}
  {collins}
  {sources}
</div>
<script>
{dismiss}
{escape}
{fit}
</script>
</body>
</html>"#,
        theme = theme_css(),
        word = html_escape::encode_text(word),
        phon = phon_html,
        audio = audio_html,
        defs = defs_html,
        examples = examples_html,
        collins = collins_html,
        sources = sources_html,
        dismiss = dismiss,
        escape = escape,
        fit = fit,
    )
}

fn build_card_html(
    source: Option<&str>,
    translated: &str,
    dismiss_ms: u64,
    auto: bool,
    pin_label: Option<&str>,
) -> String {
    // Multi-engine display_text() emits `[engine] text` lines. Render each as a
    // labeled block instead of one blob so 划词 can compare engines at a glance.
    let translated_html = render_translated_lines(translated);
    let src_html = source
        .map(|s| {
            format!(
                r#"<div class="source">{}</div>"#,
                html_escape::encode_text(s)
            )
        })
        .unwrap_or_default();
    let dismiss = if auto && dismiss_ms > 0 {
        dismiss_script(dismiss_ms)
    } else {
        String::new()
    };
    let escape = escape_script();
    let fit = fit_script();
    // Tier4-3: resize listener for pinned cards. Reports new window size
    // to the backend so PinSlot metadata stays in sync. Debounced 200ms
    // to avoid flooding the backend during drag.
    let resize_script = if let Some(label) = pin_label {
        format!(
            r"(function(){{
  var t=null;
  function sendSize(){{
    t=null;
    var w=window.innerWidth||document.documentElement.clientWidth;
    var h=window.innerHeight||document.documentElement.clientHeight;
    if(window.__moonInvoke){{
      window.__moonInvoke('update_pinned_card_size',{{label:{lbl},width:w,height:h}});
    }}
  }}
  window.addEventListener('resize',function(){{
    if(t)clearTimeout(t);
    t=setTimeout(sendSize,200);
  }});
}})();",
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
  width:100%; min-height:100%; margin:0;
  background: var(--bg) !important;
  font-family: "Segoe UI","Microsoft YaHei",sans-serif;
}}
html::-webkit-scrollbar, body::-webkit-scrollbar {{ display:none; }}
.card {{
  width: 100%;
  min-height: 100%;
  background: var(--bg-elev);
  color: var(--fg);
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: var(--shadow);
  padding: 11px 13px;
  font-size: 13px;
  line-height: 1.5;
  user-select: text;
  display: flex;
  flex-direction: column;
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
  word-break: break-word;
  overflow: visible;
}}
.mt-line {{ margin-top: 4px; }}
.mt-line:first-child {{ margin-top: 0; }}
.mt-badge {{
  display:inline-block; font-size:10px; font-weight:600;
  color: var(--accent); border: 1px solid var(--accent);
  border-radius: 4px; padding: 0 5px; margin-right: 6px;
  vertical-align: middle; white-space: nowrap;
}}
</style>
</head>
<body>
<div class="card">
  {src}
  <div class="translated">{body}</div>
</div>
<script>
{invoke}
{dismiss}
{resize}
{escape}
{fit}
</script>
</body>
</html>"#,
        theme = theme_css(),
        src = src_html,
        body = translated_html,
        invoke = INVOKE,
        dismiss = dismiss,
        resize = resize_script,
        escape = escape,
        fit = fit,
    )
}

/// Split a `display_text()` string into lines and wrap each `[engine] text`
/// line in a badge + text pair. Plain lines pass through unchanged.
fn render_translated_lines(text: &str) -> String {
    let mut out = String::new();
    for line in text.split('\n') {
        let line = line.trim_end();
        if let Some(rest) = line.strip_prefix('[') {
            if let Some((engine, body)) = rest.split_once(']') {
                let engine = engine.trim();
                let body = body.trim();
                if !engine.is_empty() && !body.is_empty() {
                    out.push_str(&format!(
                        r#"<div class="mt-line"><span class="mt-badge">{}</span><span class="mt-text">{}</span></div>"#,
                        html_escape::encode_text(engine),
                        html_escape::encode_text(body)
                    ));
                    continue;
                }
            }
        }
        out.push_str(&format!(
            r#"<div class="mt-line">{}</div>"#,
            html_escape::encode_text(line)
        ));
    }
    out
}
