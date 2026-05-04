pub mod api_server;
pub mod blacklist;
pub mod cache;
pub mod commands;
pub mod config;
pub mod dictionary;
pub mod engine;
pub mod epub_reader;
pub mod glossary;
pub mod lang_detect;
pub mod memory;
pub mod pdf;
pub mod plugin;
pub mod post_process;
pub mod services;
pub mod subtitle;
pub mod tts;

use cache::TranslationCache;
use config::AppConfig;
use glossary::Glossary;
use memory::{HistoryStore, WordBookStore};
use post_process::PostProcessor;
use services::TranslationService;
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};
use tokio::sync::{Mutex, RwLock};

pub struct AppState {
    pub config: Arc<Mutex<AppConfig>>,
    pub history: Arc<Mutex<HistoryStore>>,
    pub wordbook: Arc<Mutex<WordBookStore>>,
    pub post_processor: Arc<Mutex<PostProcessor>>,
    pub engine_router: Arc<RwLock<engine::Router>>,
    pub cache: Arc<TranslationCache>,
    pub glossary: Arc<Mutex<Glossary>>,
    pub translation_service: Arc<TranslationService>,
}

pub fn run() {
    let config = AppConfig::load();
    let history = HistoryStore::load();
    let wordbook = WordBookStore::load();
    let post_processor = PostProcessor::load();
    let glossary = Glossary::load();
    let engine_router = Arc::new(RwLock::new(engine::Router::new(&config)));
    let cache = Arc::new(TranslationCache::new(1000));

    let config_arc = Arc::new(Mutex::new(config));
    let history_arc = Arc::new(Mutex::new(history));
    let glossary_arc = Arc::new(Mutex::new(glossary));

    // Create TranslationService
    let translation_service = Arc::new(TranslationService::new(
        config_arc.clone(),
        glossary_arc.clone(),
        history_arc.clone(),
        cache.clone(),
        engine_router.clone(),
    ));

    let state = AppState {
        config: config_arc,
        history: history_arc,
        wordbook: Arc::new(Mutex::new(wordbook)),
        post_processor: Arc::new(Mutex::new(post_processor)),
        engine_router,
        cache,
        glossary: glossary_arc,
        translation_service,
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .manage(state)
        .setup(|app| {
            // Restore window position from config
            if let Some(window) = app.get_webview_window("main") {
                let app_state = app.state::<AppState>();
                let config = app_state.config.blocking_lock();
                if let (Some(x), Some(y), Some(w), Some(h)) = (config.window_x, config.window_y, config.window_width, config.window_height) {
                    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(x as i32, y as i32)));
                    let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(w as u32, h as u32)));
                }
            }

            // Create system tray menu
            let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let ocr = MenuItem::with_id(app, "ocr", "OCR截图翻译", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&show, &ocr, &settings, &quit])?;

            // Create system tray
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("Moon Translator")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "ocr" => {
                        // Trigger OCR via event
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = window.emit("trigger-ocr", ());
                        }
                    }
                    "settings" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = window.emit("navigate", "settings");
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::DoubleClick {
                        button: tauri::tray::MouseButton::Left,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Register global shortcuts from config
            use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

            // Helper to parse hotkey string like "Ctrl+Shift+T" into Shortcut
            fn parse_hotkey(hotkey: &str) -> Option<Shortcut> {
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
                            code = match key.to_uppercase().as_str() {
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
                            };
                        }
                    }
                }

                code.map(|c| Shortcut::new(Some(modifiers), c))
            }

            // Read hotkeys from config
            let hotkey_config = {
                let config = app.state::<AppState>().config.clone();
                let config = config.blocking_lock();
                config.hotkeys.clone()
            };

            // Register OCR hotkey
            if let Some(shortcut_ocr) = parse_hotkey(&hotkey_config.ocr_translate) {
                let app_handle = app.handle().clone();
                let _ = app.global_shortcut().on_shortcut(shortcut_ocr, move |_app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = window.emit("trigger-ocr", ());
                        }
                    }
                });
            }

            // Register show window hotkey
            if let Some(shortcut_show) = parse_hotkey(&hotkey_config.show_window) {
                let app_handle = app.handle().clone();
                let _ = app.global_shortcut().on_shortcut(shortcut_show, move |_app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                });
            }

            // Register translate selection hotkey
            if let Some(shortcut_translate) = parse_hotkey(&hotkey_config.translate_selection) {
                let app_handle = app.handle().clone();
                let _ = app.global_shortcut().on_shortcut(shortcut_translate, move |_app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.emit("trigger-translate-selection", ());
                        }
                    }
                });
            }

            // Register replace translate hotkey
            if let Some(shortcut_replace) = parse_hotkey(&hotkey_config.replace_translate) {
                let app_handle = app.handle().clone();
                let _ = app.global_shortcut().on_shortcut(shortcut_replace, move |_app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.emit("trigger-replace-translate", ());
                        }
                    }
                });
            }

            // Start API server if enabled
            let api_state = api_server::ApiState::from(&*app.state::<AppState>());
            let api_config = app.state::<AppState>().config.clone();
            let api_port = {
                let config = api_config.blocking_lock();
                config.api_server_port
            };
            let api_enabled = {
                let config = api_config.blocking_lock();
                config.api_server_enabled
            };

            if api_enabled {
                tokio::spawn(async move {
                    if let Err(e) = api_server::start_server(api_port, api_state).await {
                        eprintln!("API server error: {}", e);
                    }
                });
                println!("API server starting on port {}", api_port);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::translate::translate,
            commands::translate::translate_stream,
            commands::translate::translate_embedded,
            commands::translate::start_clipboard_monitor,
            commands::translate::stop_clipboard_monitor,
            commands::translate::translate_selection_with_text,
            commands::translate::replace_translate,
            commands::translate::replace_text_in_app,
            commands::translate::back_translate,
            commands::translate::polish_translation,
            commands::translate::detect_language,
            commands::translate::lookup_dictionary,
            commands::window::create_overlay,
            commands::window::close_overlay,
            commands::window::hide_main_window,
            commands::window::show_main_window,
            commands::window::translate_selection,
            commands::window::get_cursor_position,
            commands::window::toggle_always_on_top,
            commands::window::get_always_on_top,
            commands::window::move_window_to_cursor,
            commands::window::set_overlay_click_through,
            commands::window::pin_overlay,
            commands::window::move_overlay,
            commands::window::resize_overlay,
            commands::config_cmd::get_config,
            commands::config_cmd::save_config,
            commands::config_cmd::save_window_position,
            commands::config_cmd::get_window_position,
            commands::config_cmd::get_api_server_status,
            commands::config_cmd::export_config_json,
            commands::config_cmd::import_config_json,
            commands::config_cmd::get_translation_blacklist,
            commands::config_cmd::update_translation_blacklist,
            commands::history_cmd::get_history,
            commands::history_cmd::clear_history,
            commands::history_cmd::delete_history_item,
            commands::history_cmd::batch_delete_history,
            commands::cache_cmd::clear_cache,
            commands::cache_cmd::cache_size,
            commands::capture::capture_screen,
            commands::capture::capture_full_screen,
            commands::glossary_cmd::get_glossary,
            commands::glossary_cmd::get_all_glossary,
            commands::glossary_cmd::add_glossary_entry,
            commands::glossary_cmd::remove_glossary_entry,
            commands::tools_cmd::transform_variable_name,
            commands::tools_cmd::cycle_variable_name,
            commands::tts_cmd::text_to_speech,
            commands::tts_cmd::get_tts_voices,
            commands::wordbook_cmd::get_wordbook,
            commands::wordbook_cmd::add_wordbook_entry,
            commands::wordbook_cmd::update_wordbook_note,
            commands::wordbook_cmd::delete_wordbook_entry,
            commands::wordbook_cmd::batch_delete_wordbook,
            commands::wordbook_cmd::clear_wordbook,
            commands::wordbook_cmd::search_wordbook,
            commands::wordbook_cmd::export_wordbook_csv,
            commands::pdf_cmd::open_pdf,
            commands::pdf_cmd::translate_pdf,
            commands::epub_cmd::open_epub,
            commands::epub_cmd::translate_epub,
            commands::subtitle_cmd::open_subtitle,
            commands::subtitle_cmd::translate_subtitle,
            commands::subtitle_cmd::export_subtitle_file,
            commands::subtitle_cmd::translate_subtitle_text,
            commands::post_process_cmd::get_post_process_config,
            commands::post_process_cmd::update_post_process_config,
            commands::post_process_cmd::add_replacement_rule,
            commands::post_process_cmd::remove_replacement_rule,
            commands::post_process_cmd::update_replacement_rule,
            commands::post_process_cmd::test_post_process,
            commands::plugin_cmd::get_plugins,
            commands::plugin_cmd::set_plugin_enabled,
            commands::plugin_cmd::get_plugins_dir,
            commands::plugin_cmd::open_plugins_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
