// C ABI bridge over bergamot-translator (AsyncService / TranslationModel / Response).
// Consumed from Rust via hand-written extern "C" declarations (no bindgen/cxx).
// All allocation crossing the boundary is managed here; strings returned to Rust
// are heap-allocated and freed with bg_string_free.
//
// Threading: bg_translate / bg_pivot are synchronous (blocking) wrappers over the
// async service. They are safe to call from any thread but only one outstanding
// call should be made per service at a time; the Rust side serializes via a Mutex.

#ifndef BERGAMOT_BRIDGE_H
#define BERGAMOT_BRIDGE_H

#ifdef __cplusplus
extern "C" {
#endif

// Opaque handles.
typedef void* BgService;
typedef void* BgModel;

// Service lifecycle. num_workers = number of marian worker threads.
BgService bg_service_create(int num_workers, int cache_size);
void bg_service_destroy(BgService svc);

// Load a model from a marian config file (config paths resolve relative to it).
// Returns NULL on failure (check stderr).
BgModel bg_model_load(BgService svc, const char* config_path);
void bg_model_destroy(BgModel model);

// Translate a single string through one model. Returns a heap string to free
// with bg_string_free, or NULL on failure.
char* bg_translate(BgService svc, BgModel model, const char* text);

// Pivot translation: first model (A->pivot) then second (pivot->B).
char* bg_pivot(BgService svc, BgModel first, BgModel second, const char* text);

void bg_string_free(char* p);

#ifdef __cplusplus
}  // extern "C"
#endif

#endif  // BERGAMOT_BRIDGE_H
