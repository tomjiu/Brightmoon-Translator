//! C ABI bridge over the native bergamot-translator static library.
//!
//! The native side (`src-tauri/native/bergamot_bridge.cpp`) is compiled by
//! `scripts/build-bergamot-native.ps1` into `bergamot_bridge.lib` together
//! with its transitive static libs (bergamot-translator, marian, sentencepiece,
//! yaml-cpp, intgemm, ssplit, pcre2, onnx-sgemm) under `src-tauri/native/lib/`.
//! `build.rs` enables the `bergamot_native` cfg when those libs are present;
//! otherwise this module degrades to a stub so `cargo check`/tests stay green
//! on machines that have not built the native stack yet.
//!
//! All `unsafe` FFI is confined to this module: callers use the `NativeService`
//! / `NativeModel` wrappers. Raw handles are never exposed outside it.

use std::collections::HashMap;
#[cfg(bergamot_native)]
use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::{Arc, Mutex};

// Raw C ABI surface, linked from `bergamot_bridge.lib`.
#[cfg(bergamot_native)]
unsafe extern "C" {
    fn bg_service_create(num_workers: i32, cache_size: i32) -> *mut c_void;
    fn bg_service_destroy(svc: *mut c_void);
    fn bg_model_load(svc: *mut c_void, config_path: *const c_char) -> *mut c_void;
    fn bg_model_destroy(model: *mut c_void);
    fn bg_translate(svc: *mut c_void, model: *mut c_void, text: *const c_char) -> *mut c_char;
    fn bg_pivot(
        svc: *mut c_void,
        first: *mut c_void,
        second: *mut c_void,
        text: *const c_char,
    ) -> *mut c_char;
    fn bg_string_free(p: *mut c_char);
}

/// Handle to the native bergamot `AsyncService` (owns the worker-thread pool).
#[cfg(bergamot_native)]
pub struct NativeService {
    handle: *mut c_void,
}

/// Handle to one loaded `TranslationModel`.
#[cfg(bergamot_native)]
pub struct NativeModel {
    handle: *mut c_void,
}

/// Stub used when native libs were not built.
#[cfg(not(bergamot_native))]
pub struct NativeService;

/// Stub used when native libs were not built.
#[cfg(not(bergamot_native))]
pub struct NativeModel;

#[cfg(bergamot_native)]
impl NativeService {
    /// Create a service with `num_workers` worker threads.
    ///
    /// # Errors
    ///
    /// Returns an error when the native service could not be created.
    pub fn new(num_workers: i32) -> anyhow::Result<Self> {
        let handle = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // SAFETY: bg_service_create allocates the service and returns an
            // owning handle; a null handle signals failure.
            unsafe { bg_service_create(num_workers, 0) }
        }))
        .map_err(|_| anyhow::anyhow!("panic while creating native bergamot service"))?;
        if handle.is_null() {
            anyhow::bail!("failed to create native bergamot service");
        }
        Ok(Self { handle })
    }

    /// Load a model from its marian `config.yml` on disk.
    ///
    /// # Errors
    ///
    /// Returns an error when the config is missing, malformed, or the model
    /// files it references cannot be loaded.
    pub fn load_model(&self, config_path: &str) -> anyhow::Result<NativeModel> {
        let path = CString::new(config_path)
            .map_err(|_| anyhow::anyhow!("model config path contains a NUL byte"))?;
        let handle = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // SAFETY: handle is a valid service handle created by `new`; path
            // is a NUL-terminated copy of `config_path`.
            unsafe { bg_model_load(self.handle, path.as_ptr()) }
        }))
        .map_err(|_| anyhow::anyhow!("panic while loading model from `{config_path}`"))?;
        if handle.is_null() {
            anyhow::bail!("failed to load model from `{config_path}`");
        }
        Ok(NativeModel { handle })
    }

    /// Translate `text` through a single loaded model.
    ///
    /// # Errors
    ///
    /// Returns an error when the bridge returns a null string (inference error)
    /// or the text contains a NUL byte.
    pub fn translate(&self, model: &NativeModel, text: &str) -> anyhow::Result<String> {
        let ctext = CString::new(text)
            .map_err(|_| anyhow::anyhow!("translation text contains a NUL byte"))?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            // SAFETY: handles are valid; ctext is NUL-terminated. The bridge
            // returns a malloc'd buffer owned by us, freed below.
            let out = bg_translate(self.handle, model.handle, ctext.as_ptr());
            if out.is_null() {
                return Err(anyhow::anyhow!("translation failed"));
            }
            let s = CStr::from_ptr(out).to_string_lossy().into_owned();
            // SAFETY: `out` came from bg_translate and must be released via
            // bg_string_free exactly once.
            bg_string_free(out);
            Ok::<_, anyhow::Error>(s)
        }))
        .map_err(|_| anyhow::anyhow!("panic inside native translation bridge"))??;
        Ok(result)
    }

    /// Pivot-translate `text`: first model translates the source language into
    /// English, the second model translates English into the target language.
    ///
    /// # Errors
    ///
    /// Returns an error when the bridge returns a null string (inference error)
    /// or the text contains a NUL byte.
    pub fn pivot(
        &self,
        first: &NativeModel,
        second: &NativeModel,
        text: &str,
    ) -> anyhow::Result<String> {
        let ctext = CString::new(text)
            .map_err(|_| anyhow::anyhow!("translation text contains a NUL byte"))?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            // SAFETY: handles are valid; ctext is NUL-terminated.
            let out = bg_pivot(self.handle, first.handle, second.handle, ctext.as_ptr());
            if out.is_null() {
                return Err(anyhow::anyhow!("pivot translation failed"));
            }
            let s = CStr::from_ptr(out).to_string_lossy().into_owned();
            // SAFETY: `out` came from bg_pivot and must be released via
            // bg_string_free exactly once.
            bg_string_free(out);
            Ok::<_, anyhow::Error>(s)
        }))
        .map_err(|_| anyhow::anyhow!("panic inside native pivot bridge"))??;
        Ok(result)
    }
}

#[cfg(bergamot_native)]
impl Drop for NativeService {
    fn drop(&mut self) {
        // SAFETY: handle is valid and this is the last reference (Drop).
        unsafe { bg_service_destroy(self.handle) };
    }
}

// SAFETY: the C++ service behind this handle is only touched through the
// bridge functions; `AsyncService` is internally synchronized (worker pool +
// response cache), and callers serialize model access via the engine's model
// cache mutex. The handle is never exposed outside this module.
#[cfg(bergamot_native)]
unsafe impl Send for NativeService {}
#[cfg(bergamot_native)]
unsafe impl Sync for NativeService {}

#[cfg(bergamot_native)]
impl Drop for NativeModel {
    fn drop(&mut self) {
        // SAFETY: handle is valid and this is the last reference (Drop).
        unsafe { bg_model_destroy(self.handle) };
    }
}

// SAFETY: see the NativeService Send/Sync justification above. `TranslationModel`
// is safe to call from multiple threads (each translate submits to the service's
// pool); access is additionally serialized by the engine's model cache mutex.
#[cfg(bergamot_native)]
unsafe impl Send for NativeModel {}
#[cfg(bergamot_native)]
unsafe impl Sync for NativeModel {}

#[cfg(not(bergamot_native))]
impl NativeService {
    /// Create a service. Always fails when native libs are not built.
    ///
    /// # Errors
    ///
    /// Returns an error explaining that `scripts/build-bergamot-native.ps1`
    /// must be run first.
    pub fn new(_num_workers: i32) -> anyhow::Result<Self> {
        anyhow::bail!(
            "native bergamot libs are not built; run scripts/build-bergamot-native.ps1"
        )
    }

    /// Load a model. Always fails when native libs are not built.
    ///
    /// # Errors
    ///
    /// See [`NativeService::new`].
    pub fn load_model(&self, _config_path: &str) -> anyhow::Result<NativeModel> {
        anyhow::bail!(
            "native bergamot libs are not built; run scripts/build-bergamot-native.ps1"
        )
    }

    /// Translate. Always fails when native libs are not built.
    ///
    /// # Errors
    ///
    /// See [`NativeService::new`].
    pub fn translate(&self, _model: &NativeModel, _text: &str) -> anyhow::Result<String> {
        anyhow::bail!(
            "native bergamot libs are not built; run scripts/build-bergamot-native.ps1"
        )
    }

    /// Pivot-translate. Always fails when native libs are not built.
    ///
    /// # Errors
    ///
    /// See [`NativeService::new`].
    pub fn pivot(
        &self,
        _first: &NativeModel,
        _second: &NativeModel,
        _text: &str,
    ) -> anyhow::Result<String> {
        anyhow::bail!(
            "native bergamot libs are not built; run scripts/build-bergamot-native.ps1"
        )
    }
}

/// Loaded-model cache shared by the engine: pair id -> model handle.
pub type ModelCache = Mutex<HashMap<String, Arc<NativeModel>>>;

/// Load a model into `cache` (or reuse the cached handle) and return it.
/// Must be called from a blocking context (model load is synchronous I/O).
///
/// # Errors
///
/// Returns an error when the pair's `config.yml` is missing or the native
/// service fails to load it.
pub fn load_model_cached(
    model_dir: &std::path::Path,
    cache: &ModelCache,
    svc: &NativeService,
    pair_id: &str,
) -> anyhow::Result<Arc<NativeModel>> {
    {
        let cache = cache.lock().map_err(|_| anyhow::anyhow!("model cache poisoned"))?;
        if let Some(model) = cache.get(pair_id) {
            return Ok(Arc::clone(model));
        }
    }
    let config_path = model_dir.join(pair_id).join("config.yml");
    if !config_path.exists() {
        anyhow::bail!(
            "model `{pair_id}` is not downloaded (missing `{}`)",
            config_path.display()
        );
    }
    let model = Arc::new(svc.load_model(&config_path.to_string_lossy())?);
    let mut cache = cache.lock().map_err(|_| anyhow::anyhow!("model cache poisoned"))?;
    cache.insert(pair_id.to_string(), Arc::clone(&model));
    Ok(model)
}

#[cfg(test)]
mod tests {
    #[cfg(bergamot_native)]
    use super::*;

    /// Real model dir used by the spike (also the Task 8 E2E fixture).
    #[cfg(bergamot_native)]
    const SPIKE_CONFIG_EN_ZH: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../spike/bergamot-cjk/config.enzh.yml");

    #[test]
    #[cfg(bergamot_native)]
    fn service_lifecycle_create_destroy() {
        let svc = NativeService::new(1).expect("service should be creatable");
        drop(svc);
    }

    #[test]
    #[cfg(bergamot_native)]
    fn translate_direct_en_zh_returns_chinese() {
        let svc = NativeService::new(1).expect("service should be creatable");
        let model = svc
            .load_model(SPIKE_CONFIG_EN_ZH)
            .expect("spike en-zh model should load");
        let out = svc
            .translate(&model, "The moon is bright.")
            .expect("translation should succeed");
        assert!(out.contains("月"), "expected Chinese output, got: {out}");
        assert!(!out.contains('['), "unexpected pipeline text in output: {out}");
    }
}
