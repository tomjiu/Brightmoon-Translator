use crate::config::HotkeyConfig;
use crate::overlay;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

/// Parse a hotkey string like "Ctrl+Shift+T" into a Shortcut.
pub fn parse_hotkey(hotkey: &str) -> Option<Shortcut> {
    let parts: Vec<&str> = hotkey.split('+').map(|s| s.trim()).collect();
    let mut modifiers = Modifiers::empty();
    let mut code = None;

    for part in &parts {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "shift" => modifiers |= Modifiers::SHIFT,
            "alt" => modifiers |= Modifiers::ALT,
            "super" | "win" | "meta" => modifiers |= Modifiers::SUPER,
            key => {
                code = parse_key_code(key);
            }
        }
    }

    code.map(|c| Shortcut::new(Some(modifiers), c))
}

fn parse_key_code(key: &str) -> Option<Code> {
    match key.to_uppercase().as_str() {
        "A" => Some(Code::KeyA),
        "B" => Some(Code::KeyB),
        "C" => Some(Code::KeyC),
        "D" => Some(Code::KeyD),
        "E" => Some(Code::KeyE),
        "F" => Some(Code::KeyF),
        "G" => Some(Code::KeyG),
        "H" => Some(Code::KeyH),
        "I" => Some(Code::KeyI),
        "J" => Some(Code::KeyJ),
        "K" => Some(Code::KeyK),
        "L" => Some(Code::KeyL),
        "M" => Some(Code::KeyM),
        "N" => Some(Code::KeyN),
        "O" => Some(Code::KeyO),
        "P" => Some(Code::KeyP),
        "Q" => Some(Code::KeyQ),
        "R" => Some(Code::KeyR),
        "S" => Some(Code::KeyS),
        "T" => Some(Code::KeyT),
        "U" => Some(Code::KeyU),
        "V" => Some(Code::KeyV),
        "W" => Some(Code::KeyW),
        "X" => Some(Code::KeyX),
        "Y" => Some(Code::KeyY),
        "Z" => Some(Code::KeyZ),
        "0" => Some(Code::Digit0),
        "1" => Some(Code::Digit1),
        "2" => Some(Code::Digit2),
        "3" => Some(Code::Digit3),
        "4" => Some(Code::Digit4),
        "5" => Some(Code::Digit5),
        "6" => Some(Code::Digit6),
        "7" => Some(Code::Digit7),
        "8" => Some(Code::Digit8),
        "9" => Some(Code::Digit9),
        "F1" => Some(Code::F1),
        "F2" => Some(Code::F2),
        "F3" => Some(Code::F3),
        "F4" => Some(Code::F4),
        "F5" => Some(Code::F5),
        "F6" => Some(Code::F6),
        "F7" => Some(Code::F7),
        "F8" => Some(Code::F8),
        "F9" => Some(Code::F9),
        "F10" => Some(Code::F10),
        "F11" => Some(Code::F11),
        "F12" => Some(Code::F12),
        "SPACE" => Some(Code::Space),
        "ENTER" => Some(Code::Enter),
        "TAB" => Some(Code::Tab),
        "ESCAPE" | "ESC" => Some(Code::Escape),
        "BACKSPACE" => Some(Code::Backspace),
        "DELETE" | "DEL" => Some(Code::Delete),
        "INSERT" | "INS" => Some(Code::Insert),
        "HOME" => Some(Code::Home),
        "END" => Some(Code::End),
        "PAGEUP" => Some(Code::PageUp),
        "PAGEDOWN" => Some(Code::PageDown),
        "UP" => Some(Code::ArrowUp),
        "DOWN" => Some(Code::ArrowDown),
        "LEFT" => Some(Code::ArrowLeft),
        "RIGHT" => Some(Code::ArrowRight),
        _ => None,
    }
}

/// Register all global hotkeys from config.
/// Must be called inside Tauri setup() where AppHandle is available.
pub fn register_all(app: &tauri::App, config: &HotkeyConfig) {
    // OCR hotkey
    if let Some(shortcut) = parse_hotkey(&config.ocr_translate) {
        let app_handle = app.handle().clone();
        let _ = app.global_shortcut().on_shortcut(
            shortcut,
            move |_app, _shortcut, event| {
                if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.emit("trigger-ocr-screenshot", ());
                    }
                }
            },
        );
    }

    // Show window hotkey
    if let Some(shortcut) = parse_hotkey(&config.show_window) {
        let app_handle = app.handle().clone();
        let _ = app.global_shortcut().on_shortcut(
            shortcut,
            move |_app, _shortcut, event| {
                if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            },
        );
    }

    // Translate selection hotkey
    if let Some(shortcut) = parse_hotkey(&config.translate_selection) {
        let app_handle = app.handle().clone();
        let _ = app.global_shortcut().on_shortcut(
            shortcut,
            move |_app, _shortcut, event| {
                if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.emit("trigger-translate-selection", ());
                    }
                }
            },
        );
    }

    // Replace translate hotkey
    if let Some(shortcut) = parse_hotkey(&config.replace_translate) {
        let app_handle = app.handle().clone();
        let _ = app.global_shortcut().on_shortcut(
            shortcut,
            move |_app, _shortcut, event| {
                if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.emit("trigger-replace-translate", ());
                    }
                }
            },
        );
    }

    // Overlay click-through toggle hotkey
    if let Some(shortcut) = parse_hotkey(&config.toggle_overlay_click_through) {
        let app_handle = app.handle().clone();
        let _ = app.global_shortcut().on_shortcut(
            shortcut,
            move |_app, _shortcut, event| {
                if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    overlay::interaction::disable_click_through_and_focus(&app_handle);
                }
            },
        );
    }
}
