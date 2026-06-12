use crate::cache::TranslationCache;
use crate::capabilities::{HookMonitor, TargetAppDetector, WindowsTargetAppDetector};
use crate::config::AppConfig;
use crate::engine;
use crate::glossary::Glossary;
use crate::hook_profile::HookProfileManager;
use crate::memory::{HistoryStore, WordBookStore};
use crate::metrics::MetricsCollector;
use crate::overlay::{FollowController, OverlayHttpServer};
use crate::post_process::PostProcessor;
use crate::pre_process::PreProcessor;
use crate::selection;
use crate::services::TranslationService;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// Translation engine orchestration context
pub struct TranslationContext {
    pub service: Arc<TranslationService>,
    pub engine_router: Arc<RwLock<engine::Router>>,
    pub cache: Arc<TranslationCache>,
    pub glossary: Arc<Mutex<Glossary>>,
    pub metrics: Arc<MetricsCollector>,
}

/// Document processing context (history, wordbook, post-processing)
pub struct DocumentContext {
    pub history: Arc<Mutex<HistoryStore>>,
    pub wordbook: Arc<Mutex<WordBookStore>>,
    pub post_processor: Arc<Mutex<PostProcessor>>,
    pub pre_processor: Arc<PreProcessor>,
}

/// Overlay window management context
pub struct OverlayContext {
    pub follow_controller: Arc<FollowController>,
    /// HTTP server for serving overlay content (avoids data URI overhead)
    pub http_server: Arc<RwLock<Option<OverlayHttpServer>>>,
}

/// Hook monitor context for foreground window text capture
pub struct HookContext {
    pub hook_monitor: Arc<Mutex<HookMonitor>>,
    pub profiles: Arc<HookProfileManager>,
}

/// System-level context (config, selection, app detection)
pub struct SystemContext {
    pub config: Arc<Mutex<AppConfig>>,
    pub selection_manager: Arc<selection::SelectionProviderManager>,
    pub app_detector: Arc<dyn TargetAppDetector>,
}

/// All sub-contexts bundled together
pub struct Contexts {
    pub translation: TranslationContext,
    pub document: DocumentContext,
    pub overlay: OverlayContext,
    pub hook: HookContext,
    pub system: SystemContext,
}

/// Initialize all sub-contexts from config.
pub async fn build_contexts() -> Contexts {
    let config = AppConfig::load();
    let history = HistoryStore::load();
    let wordbook = WordBookStore::load();
    let post_processor = PostProcessor::load();
    let pre_processor = PreProcessor::load();
    let glossary = Glossary::load().await;
    let engine_router = Arc::new(RwLock::new(engine::Router::new(&config)));
    let cache = Arc::new(TranslationCache::new(1000));
    let metrics = Arc::new(MetricsCollector::new());

    let config_arc = Arc::new(Mutex::new(config));
    let history_arc = Arc::new(Mutex::new(history));
    let glossary_arc = Arc::new(Mutex::new(glossary));

    let pre_processor_arc = Arc::new(pre_processor);
    let post_processor_arc = Arc::new(Mutex::new(post_processor));

    let translation_service = Arc::new(TranslationService::new(
        config_arc.clone(),
        glossary_arc.clone(),
        history_arc.clone(),
        cache.clone(),
        engine_router.clone(),
        metrics.clone(),
        pre_processor_arc.clone(),
        post_processor_arc.clone(),
    ));

    let selection_manager = Arc::new(selection::SelectionProviderManager::with_defaults());
    let app_detector: Arc<dyn TargetAppDetector> = Arc::new(WindowsTargetAppDetector::new());
    let follow_controller = Arc::new(FollowController::new());
    let hook_monitor = Arc::new(Mutex::new(HookMonitor::new()));
    let hook_profiles = Arc::new(HookProfileManager::load());

    Contexts {
        translation: TranslationContext {
            service: translation_service,
            engine_router,
            cache,
            glossary: glossary_arc,
            metrics,
        },
        document: DocumentContext {
            history: history_arc,
            wordbook: Arc::new(Mutex::new(wordbook)),
            post_processor: post_processor_arc,
            pre_processor: pre_processor_arc,
        },
        overlay: OverlayContext {
            follow_controller,
            http_server: Arc::new(RwLock::new(None)),
        },
        hook: HookContext {
            hook_monitor,
            profiles: hook_profiles,
        },
        system: SystemContext {
            config: config_arc,
            selection_manager,
            app_detector,
        },
    }
}
