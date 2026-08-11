// C ABI bridge over bergamot-translator. See bergamot_bridge.h for contract.
// Include paths: -I src-tauri/native/include (bergamot headers),
//               -I src-tauri/native/include/marian (marian-dev headers).

#include "translator/parser.h"
#include "translator/response.h"
#include "translator/service.h"
#include "bergamot_bridge.h"

#include <cstdlib>
#include <cstring>
#include <future>
#include <memory>
#include <string>
#include <utility>

using namespace marian::bergamot;

struct BgServiceImpl {
  std::unique_ptr<AsyncService> svc;
};

struct BgModelImpl {
  std::shared_ptr<TranslationModel> model;
};

namespace {

// Keep one shared service per bridge to amortize worker-thread setup; the Rust
// side owns the handle and calls bg_service_destroy on shutdown. We cannot store
// a static here because the Rust side may create multiple services.
char* copy_to_c(const std::string& s) {
  char* out = static_cast<char*>(std::malloc(s.size() + 1));
  if (!out) return nullptr;
  std::memcpy(out, s.data(), s.size());
  out[s.size()] = '\0';
  return out;
}

Response translate_sync(AsyncService& service, const std::shared_ptr<TranslationModel>& model,
                        const std::string& source) {
  std::promise<Response> promise;
  auto future = promise.get_future();
  service.translate(
      model, std::string(source),
      [&promise](Response&& response) { promise.set_value(std::move(response)); },
      ResponseOptions());
  return future.get();
}

Response pivot_sync(AsyncService& service, const std::shared_ptr<TranslationModel>& first,
                    const std::shared_ptr<TranslationModel>& second, const std::string& source) {
  std::promise<Response> promise;
  auto future = promise.get_future();
  service.pivot(
      first, second, std::string(source),
      [&promise](Response&& response) { promise.set_value(std::move(response)); },
      ResponseOptions());
  return future.get();
}

}  // namespace

extern "C" {

BgService bg_service_create(int num_workers, int cache_size) {
  AsyncService::Config config;
  config.numWorkers = static_cast<size_t>(num_workers > 0 ? num_workers : 1);
  config.cacheSize = static_cast<size_t>(cache_size > 0 ? cache_size : 0);
  auto* impl = new BgServiceImpl();
  impl->svc = std::make_unique<AsyncService>(config);
  return impl;
}

void bg_service_destroy(BgService svc) { delete static_cast<BgServiceImpl*>(svc); }

BgModel bg_model_load(BgService svc, const char* config_path) {
  if (!svc || !config_path) return nullptr;
  auto* service = static_cast<BgServiceImpl*>(svc);
  try {
    auto options = parseOptionsFromFilePath(config_path);
    auto* impl = new BgModelImpl();
    impl->model = service->svc->createCompatibleModel(options);
    return impl;
  } catch (const std::exception& e) {
    return nullptr;
  }
}

void bg_model_destroy(BgModel model) { delete static_cast<BgModelImpl*>(model); }

char* bg_translate(BgService svc, BgModel model, const char* text) {
  if (!svc || !model || !text) return nullptr;
  auto* service = static_cast<BgServiceImpl*>(svc);
  auto* impl = static_cast<BgModelImpl*>(model);
  try {
    Response response = translate_sync(*service->svc, impl->model, std::string(text));
    return copy_to_c(response.getTranslatedText());
  } catch (const std::exception& e) {
    return nullptr;
  }
}

char* bg_pivot(BgService svc, BgModel first, BgModel second, const char* text) {
  if (!svc || !first || !second || !text) return nullptr;
  auto* service = static_cast<BgServiceImpl*>(svc);
  auto* firstImpl = static_cast<BgModelImpl*>(first);
  auto* secondImpl = static_cast<BgModelImpl*>(second);
  try {
    Response response = pivot_sync(*service->svc, firstImpl->model, secondImpl->model, std::string(text));
    return copy_to_c(response.getTranslatedText());
  } catch (const std::exception& e) {
    return nullptr;
  }
}

void bg_string_free(char* p) { std::free(p); }

}  // extern "C"
