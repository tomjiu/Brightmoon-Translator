pub mod api_server;
pub mod app_context;
pub mod blacklist;
pub mod cache;
pub mod capabilities;
pub mod commands;
pub mod config;
pub mod dictionary;
pub mod engine;
pub mod epub_reader;
pub mod furigana;
pub mod glossary;
pub mod hotkey;
pub mod hook_profile;
pub mod lang_detect;
pub mod memory;
pub mod metrics;
pub mod models;
pub mod ocr_engine;
pub mod overlay;
pub mod pdf;
pub mod plugin;
pub mod post_process;
pub mod pre_process;
pub mod selection;
pub mod services;
pub mod subtitle;
pub mod tts;

use app_context::Contexts;
use capabilities::{
    DefaultInputReplacement, DefaultSelectionTranslation, InputReplacement, SelectionTranslation,
};
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};
use tokio::sync::OnceCell as TokioOnceCell;

/// Top-level application state.
/// Composed of sub-contexts for separation of concerns.
/// Commands can access either the full AppState or specific sub-contexts.
pub struct AppState {
    // Sub-contexts
    pub translation: app_context::TranslationContext,
    pub document: app_context::DocumentContext,
    pub overlay: app_context::OverlayContext,
    pub hook: app_context::HookContext,
    pub system: app_context::SystemContext,

    // Capability cells (initialized in setup() after AppHandle is available)
    pub selection_translation: TokioOnceCell<Arc<dyn SelectionTranslation>>,
    pub input_replacement: TokioOnceCell<Arc<dyn InputReplacement>>,
}

pub fn run() {
    let ctx = build_contexts();

    let state = AppState {
        translation: ctx.translation,
        document: ctx.document,
        overlay: ctx.overlay,
        hook: ctx.hook,
        system: ctx.system,
        selection_translation: TokioOnceCell::new(),
        input_replacement: TokioOnceCell::new(),
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
                let config = app_state.system.config.blocking_lock();
                if let (Some(x), Some(y), Some(w), Some(h)) = (
                    config.window_x,
                    config.window_y,
                    config.window_width,
                    config.window_height,
                ) {
                    let _ = window.set_position(tauri::Position::Physical(
                        tauri::PhysicalPosition::new(x as i32, y as i32),
                    ));
                    let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                        w as u32,
                        h as u32,
                    )));
                }
            }

            // Initialize capability implementations (needs AppHandle)
            {
                let app_state = app.state::<AppState>();
                let app_handle = app.handle().clone();

                // Initialize the follow controller with the AppHandle
                app_state.overlay.follow_controller.init(app_handle.clone());

                let sel_translation: Arc<dyn SelectionTranslation> =
                    Arc::new(DefaultSelectionTranslation::new(
                        app_state.system.selection_manager.clone(),
                        app_state.translation.service.clone(),
                        app_state.system.config.clone(),
                        app_handle,
                        app_state.system.app_detector.clone(),
                        app_state.overlay.follow_controller.clone(),
                    ));
                let inp_replacement: Arc<dyn InputReplacement> =
                    Arc::new(DefaultInputReplacement::new(
                        app_state.system.selection_manager.clone(),
                        app_state.translation.service.clone(),
                    ));
                let _ = app_state.selection_translation.set(sel_translation);
                let _ = app_state.input_replacement.set(inp_replacement);
            }

            // Create system tray menu
            let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let ocr = MenuItem::with_id(app, "ocr", "OCR截图翻译", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&show, &ocr, &settings, &quit])?;

            // Create system tray
            let icon = app.default_window_icon().cloned().unwrap_or_else(|| {
                log::warn!("No default window icon found, using empty icon");
                tauri::image::Image::new(&[], 0, 0)
            });
            let _tray = TrayIconBuilder::new()
                .icon(icon)
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
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.emit("trigger-ocr-screenshot", ());
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

            // Register global shortcuts
            let hotkey_config = {
                let app_state = app.state::<AppState>();
                let config = app_state.system.config.blocking_lock();
                config.hotkeys.clone()
            };
            hotkey::register_all(app, &hotkey_config);

            // Start API server if enabled
            start_api_server(app);

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
            commands::translate::query_tm,
            commands::translate::compare_translate,
            commands::translate::detect_language,
            commands::translate::lookup_dictionary,
            commands::window::create_overlay,
            commands::window::close_overlay,
            commands::window::hide_main_window,
            commands::window::show_main_window,
            commands::window::get_selected_text,
            commands::window::translate_selection,
            commands::window::trigger_selection_translate,
            commands::window::get_cursor_position,
            commands::window::toggle_always_on_top,
            commands::window::get_always_on_top,
            commands::window::move_window_to_cursor,
            commands::window::set_overlay_click_through,
            commands::window::pin_overlay,
            commands::window::move_overlay,
            commands::window::resize_overlay,
            commands::window::detect_foreground_app,
            commands::window::set_overlay_follow_mode,
            commands::window::refresh_overlay_position,
            commands::window::stop_overlay_follow,
            commands::window::update_overlay,
            commands::window::update_overlay_content,
            commands::window::update_overlay_position,
            commands::window::create_ocr_screenshot_selector,
            commands::window::close_ocr_screenshot_selector,
            commands::window::create_ocr_region_frame,
            commands::window::close_ocr_region_frame,
            commands::config_cmd::get_config,
            commands::config_cmd::get_default_config,
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
            commands::capture::system_ocr,
            commands::capture::system_ocr_detailed,
            commands::capture::youdao_ocr,
            commands::capture::prepare_screenshot_snapshot,
            commands::capture::load_screenshot_snapshot,
            commands::capture::crop_screenshot_snapshot,
            commands::capture::capture_screenshot_region,
            commands::capture::detect_foreground_hwnd,
            commands::capture::get_window_rect_cmd,
            commands::capture::get_window_title_cmd,
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
            commands::pre_process_cmd::get_pre_process_config,
            commands::pre_process_cmd::update_pre_process_config,
            commands::pre_process_cmd::add_pre_process_rule,
            commands::pre_process_cmd::remove_pre_process_rule,
            commands::pre_process_cmd::update_pre_process_rule,
            commands::pre_process_cmd::test_pre_process,
            commands::plugin_cmd::get_plugins,
            commands::plugin_cmd::set_plugin_enabled,
            commands::plugin_cmd::get_plugins_dir,
            commands::plugin_cmd::open_plugins_dir,
            commands::hook_cmd::start_hook_monitor,
            commands::hook_cmd::stop_hook_monitor,
            commands::hook_cmd::get_hook_monitor_status,
            commands::hook_cmd::get_foreground_window_rect,
            commands::hook_profile_cmd::get_hook_profiles,
            commands::hook_profile_cmd::get_active_hook_profile,
            commands::hook_profile_cmd::create_hook_profile,
            commands::hook_profile_cmd::update_hook_profile,
            commands::hook_profile_cmd::delete_hook_profile,
            commands::hook_profile_cmd::activate_hook_profile,
            commands::furigana_cmd::add_furigana,
            commands::furigana_cmd::add_furigana_html,
            commands::furigana_cmd::add_furigana_text,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Build all sub-contexts from configuration
fn build_contexts() -> Contexts {
    app_context::build_contexts()
}


/// Start API server if enabled in config
fn start_api_server(app: &tauri::App) {
    let app_state = app.state::<AppState>();
    let api_state = api_server::ApiState::from_app_state(&app_state);
    let config = app_state.system.config.blocking_lock();
    let api_port = config.api_server_port;
    let api_enabled = config.api_server_enabled;
    drop(config);

    if api_enabled {
        tokio::spawn(async move {
            if let Err(e) = api_server::start_server(api_port, api_state).await {
                log::error!("API server error: {}", e);
            }
        });
        log::info!("API server starting on port {}", api_port);
    }
}
