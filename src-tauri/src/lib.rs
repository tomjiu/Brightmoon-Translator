pub mod domain;
pub mod infrastructure;
pub mod skills;
pub mod tasks;

pub mod ai_enhanced;
pub mod alignment;
pub mod api_server;
pub mod app_context;
pub mod batch;
pub mod blacklist;
pub mod cache;
pub mod capabilities;
pub mod clipboard_dedupe;
pub mod collection;
pub mod commands;
pub mod config;
pub mod dictionary;
pub mod docx;
pub mod engine;
pub mod epub_reader;
pub mod error;
pub mod excel;
pub mod furigana;
pub mod glossary;
pub mod hook_inject;
pub mod hook_profile;
pub mod hotkey;
pub mod image_translate;
pub mod lang_detect;
pub mod memory;
pub mod metrics;
pub mod models;
pub mod ocr_engine;
pub mod ocr_offline;
pub mod ocr_region_consts;
pub mod overlay;
pub mod pdf;
pub mod post_process;
pub mod pptx;
pub mod pre_process;
pub mod quality;
pub mod response_check;
pub mod security;
pub mod selection;
pub mod services;
pub mod speech;
pub mod subtitle;
pub mod sync;
pub mod tbx;
pub mod tmx;
pub mod tts;

use app_context::Contexts;
use batch::BatchManager;
use capabilities::{
    DefaultInputReplacement, DefaultSelectionTranslation, InputReplacement, SelectionTranslation,
};
use infrastructure::EventStore;
use speech::SpeechState;
use sqlx::SqlitePool;
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};
use tokio::sync::{Mutex, OnceCell as TokioOnceCell};

fn clipboard_monitor_menu_text(on: bool) -> &'static str {
    if on {
        "剪贴板监听：开"
    } else {
        "剪贴板监听：关"
    }
}

fn saved_window_bounds_are_visible(
    app: &tauri::App,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> bool {
    if !x.is_finite()
        || !y.is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || width < 300.0
        || height < 200.0
    {
        return false;
    }

    let Ok(monitors) = app.available_monitors() else {
        return true;
    };

    let right = x + width;
    let bottom = y + height;

    monitors.into_iter().any(|monitor| {
        let pos = monitor.position();
        let size = monitor.size();
        let monitor_left = f64::from(pos.x);
        let monitor_top = f64::from(pos.y);
        let monitor_right = monitor_left + f64::from(size.width);
        let monitor_bottom = monitor_top + f64::from(size.height);

        let visible_width = right.min(monitor_right) - x.max(monitor_left);
        let visible_height = bottom.min(monitor_bottom) - y.max(monitor_top);

        visible_width >= 100.0 && visible_height >= 100.0
    })
}

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
    /// Auto-on-select mouseup watcher (Youdao-like)
    pub selection_auto_watch: TokioOnceCell<Arc<selection::SelectionAutoWatch>>,

    // Batch translation manager
    pub batch: Arc<BatchManager>,

    // Speech recognition state
    pub speech_state: Arc<Mutex<SpeechState>>,

    // Database for vocabulary/dictionary (ECDICT)
    pub ecdict_pool: Option<SqlitePool>,
    // Event store for learning system
    pub event_store: Option<EventStore>,
}

/// Resolve ecdict.db for both packaged and dev layouts.
pub(crate) fn resolve_ecdict_db_path() -> Option<std::path::PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Some(ref dir) = exe_dir {
        candidates.push(dir.join("dictionaries").join("ecdict.db"));
        candidates.push(dir.join("resources").join("dictionaries").join("ecdict.db"));
        candidates.push(dir.join("ecdict.db"));
        candidates.push(dir.join("resources").join("ecdict.db"));
    }
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest.join("..").join("dictionaries").join("ecdict.db"));
    candidates.push(manifest.join("dictionaries").join("ecdict.db"));
    candidates.push(std::path::PathBuf::from("dictionaries").join("ecdict.db"));
    candidates.push(std::path::PathBuf::from("ecdict.db"));
    for c in candidates {
        if c.is_file() {
            return Some(c.canonicalize().unwrap_or(c));
        }
    }
    None
}

pub fn run() {
    let ctx = tokio::runtime::Runtime::new()
        .expect("Failed to create tokio runtime")
        .block_on(build_contexts());

    // Initialize database for dictionary/vocabulary
    let (ecdict_pool, event_store) = tokio::runtime::Runtime::new()
        .expect("Failed to create tokio runtime")
        .block_on(async {
            // Connect to ECDICT — try release/portable paths first, then dev tree.
            // CARGO_MANIFEST_DIR alone fails for packaged installs (HEALTH_AUDIT B5).
            let ecdict_path = resolve_ecdict_db_path();
            let ecdict_pool = if let Some(ref path) = ecdict_path {
                let conn_str = format!("sqlite:{}", path.display().to_string().replace('\\', "/"));
                match sqlx::sqlite::SqlitePoolOptions::new()
                    .max_connections(2)
                    .connect(&conn_str)
                    .await
                {
                    Ok(pool) => {
                        tracing::info!("ECDICT database connected: {}", path.display());
                        Some(pool)
                    },
                    Err(e) => {
                        tracing::warn!("Failed to connect to ECDICT database: {}", e);
                        None
                    },
                }
            } else {
                tracing::warn!(
                    "ECDICT database not found (searched exe-dir, resources, repo dictionaries/)"
                );
                None
            };

            // Initialize vocabulary/event database
            let mut db_path = dirs::config_dir().unwrap_or_else(|| {
                tracing::warn!("config_dir not found, using current directory");
                std::path::PathBuf::from(".")
            });
            db_path.push("moontranslator");
            if let Err(e) = std::fs::create_dir_all(&db_path) {
                tracing::error!("Failed to create config directory {:?}: {}", db_path, e);
                return (ecdict_pool, None);
            }
            db_path.push("vocabulary.db");

            tracing::info!("Vocabulary database path: {}", db_path.display());

            let event_store = match sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(5)
                .connect_with(
                    sqlx::sqlite::SqliteConnectOptions::new()
                        .filename(&db_path)
                        .create_if_missing(true),
                )
                .await
            {
                Ok(pool) => {
                    let store = EventStore::from_pool(pool);
                    match store.init_schema().await {
                        Ok(()) => {
                            tracing::info!("Vocabulary database initialized");
                            Some(store)
                        },
                        Err(e) => {
                            tracing::error!("Failed to initialize database schema: {}", e);
                            None
                        },
                    }
                },
                Err(e) => {
                    tracing::error!(
                        "Failed to open vocabulary database '{}': {:#}",
                        db_path.display(),
                        e
                    );
                    None
                },
            };

            (ecdict_pool, event_store)
        });

    let state = AppState {
        translation: ctx.translation,
        document: ctx.document,
        overlay: ctx.overlay,
        hook: ctx.hook,
        system: ctx.system,
        selection_translation: TokioOnceCell::new(),
        input_replacement: TokioOnceCell::new(),
        selection_auto_watch: TokioOnceCell::new(),
        batch: Arc::new(BatchManager::new()),
        speech_state: Arc::new(Mutex::new(SpeechState::new())),
        ecdict_pool,
        event_store,
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .manage(state)
        .manage(commands::hook_inject_cmd::HookState::new())
        .setup(|app| {
            // Restore window position from config only when it is still visible on
            // the current monitor layout. Otherwise keep tauri.conf.json defaults
            // (centered window) to avoid launching off-screen after DPI/display changes.
            if let Some(window) = app.get_webview_window("main") {
                let app_state = app.state::<AppState>();
                let config = app_state.system.config.blocking_lock();
                if let (Some(x), Some(y), Some(w), Some(h)) = (
                    config.window_x,
                    config.window_y,
                    config.window_width,
                    config.window_height,
                ) {
                    if saved_window_bounds_are_visible(app, x, y, w, h) {
                        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                            w as u32,
                            h as u32,
                        )));
                        let _ = window.set_position(tauri::Position::Physical(
                            tauri::PhysicalPosition::new(x as i32, y as i32),
                        ));
                    } else {
                        tracing::warn!(
                            "Ignoring saved off-screen window bounds: ({}, {}) {}x{}",
                            x,
                            y,
                            w,
                            h
                        );
                        let _ = window.center();
                    }
                }

                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }

            // Initialize capability implementations (needs AppHandle)
            {
                let app_state = app.state::<AppState>();
                let app_handle = app.handle().clone();

                // Initialize the follow controller with the AppHandle
                app_state.overlay.follow_controller.init(app_handle.clone());

                // Start the overlay HTTP server for optimized content delivery
                let http_server_handle = app_state.overlay.http_server.clone();
                tauri::async_runtime::spawn(async move {
                    match overlay::OverlayHttpServer::start().await {
                        Ok(server) => {
                            tracing::info!(
                                "Overlay HTTP server started on port {}",
                                server.port
                            );
                            *http_server_handle.write().await = Some(server);
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to start overlay HTTP server (falling back to data URI): {}",
                                e
                            );
                        }
                    }
                });

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
                        app_state.system.config.clone(),
                    ));
                let _ = app_state.selection_translation.set(sel_translation);
                let _ = app_state.input_replacement.set(inp_replacement);

                let ux = app_state.system.config.blocking_lock().selection_ux.clone();
                let watch = Arc::new(selection::SelectionAutoWatch::new(ux));
                watch.start(app.handle().clone());
                let _ = app_state.selection_auto_watch.set(watch);
            }

            // Create system tray menu (pot-aligned: show / selection / replace / OCR / clipboard / settings / quit)
            let clipboard_monitor_on = {
                let app_state = app.state::<AppState>();
                let config = app_state.system.config.blocking_lock();
                config.clipboard_monitor
            };
            let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let selection = MenuItem::with_id(app, "selection", "划词翻译", true, None::<&str>)?;
            let replace = MenuItem::with_id(app, "replace", "替换翻译", true, None::<&str>)?;
            let ocr = MenuItem::with_id(app, "ocr", "OCR截图翻译", true, None::<&str>)?;
            let clipboard_monitor = MenuItem::with_id(
                app,
                "clipboard_monitor",
                clipboard_monitor_menu_text(clipboard_monitor_on),
                true,
                None::<&str>,
            )?;
            let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

            let menu = Menu::with_items(
                app,
                &[
                    &show,
                    &selection,
                    &replace,
                    &ocr,
                    &clipboard_monitor,
                    &settings,
                    &quit,
                ],
            )?;

            // Create system tray
            let icon = app.default_window_icon().cloned().unwrap_or_else(|| {
                tracing::warn!("No default window icon found, using empty icon");
                tauri::image::Image::new(&[], 0, 0)
            });
            let clipboard_monitor_item = clipboard_monitor.clone();
            let _tray = TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .tooltip("Moon Translator")
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "selection" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.emit("trigger-translate-selection", ());
                        }
                    }
                    "replace" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.emit("trigger-replace-translate", ());
                        }
                    }
                    "ocr" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.emit("trigger-ocr-screenshot", ());
                        }
                    }
                    "clipboard_monitor" => {
                        let app_handle = app.clone();
                        let item = clipboard_monitor_item.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app_handle.state::<AppState>();
                            let enabled = {
                                let mut config = state.system.config.lock().await;
                                config.clipboard_monitor = !config.clipboard_monitor;
                                let on = config.clipboard_monitor;
                                config.save();
                                on
                            };
                            let _ = item.set_text(clipboard_monitor_menu_text(enabled));
                            // FE syncs listener via config.clipboardMonitor + App effect
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.emit("clipboard-monitor-toggled", enabled);
                            }
                        });
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

            // Warmup OCR screenshot cache for instant capture
            // Spawns a background task 1 second after startup to pre-capture and cache screen
            // This eliminates the 1-2 second lag when user clicks OCR button
            {
                tauri::async_runtime::spawn(async move {
                    // Wait 1 second to avoid slowing down app startup
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                    tracing::info!("[OCR Warmup] Starting screenshot cache warmup...");
                    // Warmup may use cache later; force_refresh=None keeps smart cache path
                    match commands::capture::prepare_screenshot_snapshot(None).await {
                        Ok(_) => {
                            tracing::info!("[OCR Warmup] Screenshot cache warmed up successfully");
                        }
                        Err(e) => {
                            tracing::warn!("[OCR Warmup] Failed to warmup screenshot cache: {}", e);
                        }
                    }
                });
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
            commands::translate::query_tm,
            commands::translate::compare_translate,
            commands::translate::detect_language,
            commands::translate::lookup_dictionary,
            commands::window::create_overlay,
            commands::window::close_overlay,
            commands::window::hide_main_window,
            commands::window::show_main_window,
            commands::window::set_window_exclude_from_capture,
            commands::window::trigger_selection_translate,
            commands::window::trigger_dictionary_lookup,
            commands::window::get_cursor_position,
            commands::window::toggle_always_on_top,
            commands::window::get_always_on_top,
            commands::window::move_window_to_cursor,
            commands::window::set_overlay_click_through,
            commands::window::set_overlay_theme,
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
            commands::window::ocr_begin_session_hide_main,
            commands::window::ocr_end_session_show_main,
            commands::window::create_ocr_region_frame,
            commands::window::close_ocr_region_frame,
            commands::window::set_ocr_region_frame_visible,
            commands::window::set_ocr_region_frame_sampling,
            commands::window::set_ocr_region_frame_click_through,
            commands::window::move_ocr_region_frame,
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
            commands::capture::offline_ocr,
            commands::capture::prepare_screenshot_snapshot,
            commands::capture::load_screenshot_snapshot,
            commands::capture::crop_screenshot_snapshot,
            commands::capture::capture_screenshot_region,
            commands::capture::image_data_url_fingerprint,
            commands::capture::detect_foreground_hwnd,
            commands::capture::hwnd_from_point,
            commands::capture::get_window_rect_cmd,
            commands::capture::get_window_title_cmd,
            commands::capture::detect_text_regions,
            commands::hook_inject_cmd::hook_inject,
            commands::hook_inject_cmd::hook_eject,
            commands::hook_inject_cmd::hook_status,
            commands::hook_inject_cmd::hook_dll_available,
            commands::hook_inject_cmd::hook_dll_path,
            commands::hook_inject_cmd::hook_read_messages,
            commands::hook_inject_cmd::hook_process_messages,
            commands::process_list::get_process_list,
            commands::glossary_cmd::get_glossary,
            commands::glossary_cmd::get_all_glossary,
            commands::glossary_cmd::add_glossary_entry,
            commands::glossary_cmd::remove_glossary_entry,
            commands::glossary_cmd::import_glossary_tmx,
            commands::glossary_cmd::export_glossary_tmx,
            commands::glossary_cmd::import_glossary_tbx,
            commands::glossary_cmd::export_glossary_tbx,
            commands::glossary_cmd::align_text,
            commands::tools_cmd::transform_variable_name,
            commands::tools_cmd::cycle_variable_name,
            commands::tts_cmd::text_to_speech,
            commands::tts_cmd::get_tts_voices,
            commands::collection_cmd::collection_push,
            commands::collection_cmd::collection_test_target,
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
            commands::pdf_cmd::ocr_scanned_pdf,
            commands::epub_cmd::open_epub,
            commands::epub_cmd::translate_epub,
            commands::subtitle_cmd::open_subtitle,
            commands::subtitle_cmd::translate_subtitle,
            commands::subtitle_cmd::export_subtitle_file,
            commands::subtitle_cmd::translate_subtitle_text,
            commands::docx_cmd::open_docx,
            commands::docx_cmd::translate_docx,
            commands::docx_cmd::translate_docx_preview,
            commands::excel_cmd::open_excel,
            commands::excel_cmd::translate_excel,
            commands::excel_cmd::translate_excel_preview,
            commands::pptx_cmd::open_pptx,
            commands::pptx_cmd::translate_pptx,
            commands::pptx_cmd::translate_pptx_preview,
            commands::image_translate_cmd::translate_image,
            commands::image_translate_cmd::preview_image_translation,
            commands::image_translate_cmd::translate_image_base64,
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
            commands::batch_cmd::tm_export,
            commands::batch_cmd::tm_import,
            commands::batch_cmd::tm_get_stats,
            commands::batch_cmd::tm_search,
            commands::batch_cmd::tm_export_tmx,
            commands::batch_cmd::tm_import_tmx,
            commands::batch_cmd::tm_delete,
            commands::batch_cmd::tm_batch_delete,
            commands::metrics_cmd::get_metrics_summary,
            commands::metrics_cmd::get_metrics_timeline,
            commands::metrics_cmd::get_metrics_hourly_stats,
            commands::metrics_cmd::export_metrics_csv,
            commands::metrics_cmd::export_metrics_json,
            commands::metrics_cmd::clear_metrics,
            commands::metrics_cmd::prune_metrics,
            commands::ai_cmd::ai_polish_translation,
            commands::ai_cmd::ai_extract_terms,
            commands::ai_cmd::ai_learn_style,
            commands::ai_cmd::ai_context_translate,
            commands::ai_cmd::ai_multi_round_translate,
            commands::offline_cmd::get_offline_models,
            commands::offline_cmd::download_offline_model,
            commands::offline_cmd::delete_offline_model,
            commands::offline_cmd::toggle_offline_engine,
            commands::offline_cmd::update_offline_settings,
            commands::offline_cmd::generate_sample_offline_models,
            commands::offline_cmd::get_offline_status,
            commands::sync_cmd::test_webdav_connection,
            commands::sync_cmd::sync_now,
            commands::sync_cmd::get_sync_config,
            commands::sync_cmd::save_sync_config,
            commands::vocabulary_cmd::get_core_vocabulary,
            commands::vocabulary_cmd::search_core_vocabulary,
            commands::vocabulary_cmd::create_card,
            commands::vocabulary_cmd::get_card,
            commands::vocabulary_cmd::get_due_cards,
            commands::vocabulary_cmd::generate_card_content,
            commands::vocabulary_cmd::submit_review,
            commands::vocabulary_cmd::get_learning_stats,
            commands::vocabulary_cmd::study_word,
            commands::statistics_cmd::get_learning_statistics,
            commands::statistics_cmd::get_daily_activity,
            commands::statistics_cmd::get_heatmap_data,
            commands::statistics_cmd::get_weak_words,
            commands::notification_cmd::send_desktop_notification,
            commands::notification_cmd::check_daily_reminder,
            commands::notification_cmd::check_due_cards_reminder,
            commands::notification_cmd::check_milestone_celebration,
            commands::notification_cmd::check_plan_progress_reminder,
            commands::model_provider_cmd::fetch_available_models,
            commands::model_provider_cmd::test_llm_connection,
            commands::dictionary_cmd::search_word_suggestions,
            commands::dictionary_cmd::lookup_word_detail,
            commands::dictionary_cmd::lookup_word_multi_source,
            commands::dictionary_cmd::fuzzy_search_words,
            commands::dictionary_cmd::import_dictionary_data,
            commands::dictionary_cmd::check_dictionary_imported,
            commands::dictionary_cmd::ecdict_status,
            commands::learning_plan_cmd::get_exam_wordlists,
            commands::learning_plan_cmd::create_learning_plan,
            commands::learning_plan_cmd::get_learning_plans,
            commands::learning_plan_cmd::get_plan_today_words,
            commands::learning_plan_cmd::mark_word_learned,
            commands::learning_plan_cmd::delete_learning_plan,
            commands::learning_plan_cmd::import_wordlist_from_file,
            commands::learning_plan_cmd::import_wordlist_from_text,
            commands::learning_mode_cmd::generate_choice_questions,
            commands::learning_mode_cmd::generate_spelling_questions,
            commands::learning_mode_cmd::generate_cloze_questions,
            commands::learning_mode_cmd::get_swipe_cards,
            commands::learning_mode_cmd::submit_swipe_rating,
            commands::word_detail_cmd::get_word_history,
            commands::word_detail_cmd::get_fsrs_timeline,
            commands::word_detail_cmd::update_ai_content,
            commands::word_detail_cmd::get_related_words,
            commands::word_detail_cmd::get_corpus_examples,
            commands::word_detail_cmd::get_word_etymology,
            commands::data_io_cmd::export_learning_data_json,
            commands::data_io_cmd::export_anki_tsv,
            commands::data_io_cmd::import_learning_data_json,
            commands::data_io_cmd::import_wordlist_csv,
            commands::data_io_cmd::auto_backup,
            commands::data_io_cmd::write_file_content,
            commands::data_io_cmd::write_file_base64,
            commands::fsrs_optimization_cmd::get_fsrs_analysis,
            commands::fsrs_optimization_cmd::get_forgetting_curve,
            commands::fsrs_optimization_cmd::get_review_forecast,
            commands::fsrs_optimization_cmd::get_best_study_time,
            commands::fsrs_optimization_cmd::get_difficulty_distribution,
            commands::dict_optimize_cmd::get_dict_stats,
            commands::dict_optimize_cmd::export_compressed_dict,
            commands::dict_optimize_cmd::export_dict_shards,
            commands::github_export_cmd::export_for_github,
            commands::github_export_cmd::export_ai_cache_for_github,
            commands::quality_cmd::score_translation,
            commands::quality_cmd::compare_engine_quality,
            commands::speech_cmd::start_speech_recognition,
            commands::speech_cmd::stop_speech_recognition,
            commands::speech_cmd::get_speech_recognition_status,
            commands::speech_cmd::get_speech_languages,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Build all sub-contexts from configuration
async fn build_contexts() -> Contexts {
    app_context::build_contexts().await
}

/// Start API server if enabled in config
fn start_api_server(app: &tauri::App) {
    let app_state = app.state::<AppState>();
    let api_state =
        api_server::ApiState::from_app_state(&app_state).with_app_handle(app.handle().clone());
    let config = app_state.system.config.blocking_lock();
    let api_port = config.api_server_port;
    let api_enabled = config.api_server_enabled;
    drop(config);

    if api_enabled {
        tauri::async_runtime::spawn(async move {
            if let Err(e) = api_server::start_server(api_port, api_state).await {
                tracing::error!("API server error: {}", e);
            }
        });
        tracing::info!("API server starting on port {}", api_port);
    }
}
