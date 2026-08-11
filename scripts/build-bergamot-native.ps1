# Builds the native bergamot-translator static libraries needed by the Rust
# offline engine (see src-tauri/src/engine/offline/bridge.rs). Reproduces the
# Task 1 spike build (spike/VERDICT.md, windows-verified 2026-08-10).
#
# Outputs (all gitignored):
#   src-tauri/native/lib/*.lib            (bergamot + marian + transitive deps + bridge)
#   src-tauri/native/include/             (headers needed to compile the bridge)
#   src-tauri/native/bergamot_bridge.lib  (C ABI bridge over the engine)
#
# Prereqs (Windows):
#   - Visual Studio 2022 Build Tools (MSVC) — match your installed toolchain
#   - CMake 3.30.6 (portable, NOT system 4.x — 2018-era subprojects reject it)
#   - Python 3 (for marian/sentencepiece build scripts)
#   - git (submodules)
#
# Six Windows-specific fixes are applied: five to the source tree (inline
# below) and one to the *mirrored* marian header (Patch 6) that only MSVC's
# clang-exact template lookup needs when compiling the bridge.

param(
  [string]$WorkDir = "$PSScriptRoot\..\tmp\bergamot-native",
  [string]$BergamotRev = "5ae1b1e",
  [string]$Cmake = "cmake"
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path "$PSScriptRoot\.."
$SrcDir = Join-Path $WorkDir "bergamot-translator"
$BuildDir = Join-Path $WorkDir "build"
$NativeLib = Join-Path $RepoRoot "src-tauri\native\lib"
$NativeInc = Join-Path $RepoRoot "src-tauri\native\include"

New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null

# ---- 1) Clone + checkout pinned revision ----
if (-not (Test-Path $SrcDir)) {
  git clone --recurse-submodules "https://github.com/mozilla/bergamot-translator" $SrcDir
}
Push-Location $SrcDir
git checkout $BergamotRev 2>$null
git submodule update --init --recursive

# ---- 2) Apply the five Windows patches (inline, idempotent via grep guard) ----
function Guarded-Patch {
  param([string]$Path, [string]$Guard, [scriptblock]$Apply)
  $guardHit = Select-String -Path $Path -Pattern $Guard -Quiet -ErrorAction SilentlyContinue
  if ($guardHit) {
    Write-Host "  (already patched) $Path"
  } else {
    & $Apply
    Write-Host "  patched $Path"
  }
}

# Patch 1: sentencepiece protobuf-lite hash.h — VS2022 removed stdext::hash_compare
$hashH = "3rd_party\marian-dev\src\3rd_party\sentencepiece\third_party\protobuf-lite\google\protobuf\stubs\hash.h"
Guarded-Patch $hashH "google::protobuf::hash_compare" {
  $content = Get-Content $hashH -Raw
  $content = $content.Replace(
    '#include <string.h>',
    "#include <string.h>`r`n#include <functional>"
  ).Replace(
    '#  define GOOGLE_PROTOBUF_HASH_COMPARE std::hash_compare',
    '#  define GOOGLE_PROTOBUF_HASH_COMPARE google::protobuf::hash_compare'
  )
  $shim = @'
// VS2010+ removed stdext::hash_compare; provide a compatible replacement
// built on the C++11 <functional> machinery.
template <typename Key, typename KeyCompare = std::less<Key> >
class hash_compare {
 public:
  hash_compare() : key_compare_() {}
  explicit hash_compare(KeyCompare key_compare) : key_compare_(key_compare) {}
  size_t operator()(const Key& key) const { return std::hash<Key>()(key); }
  bool operator()(const Key& a, const Key& b) const { return key_compare_(a, b); }
 private:
  KeyCompare key_compare_;
};
'@
  $anchor = 'template <typename Key>'
  $content = $content.Replace($anchor, "$shim`r`n$anchor")
  Set-Content -Path $hashH -Value $content -NoNewline
}

# Patch 2: marian CMakeLists — /wd4819 for UTF-8 headers on CP936 code pages
$marianCmake = "3rd_party\marian-dev\CMakeLists.txt"
Guarded-Patch $marianCmake '/wd"4819"' {
  (Get-Content $marianCmake -Raw).Replace(
    '"/wd"4310" /wd"4324" /wd"4702" /wd"4100""',
    '"/wd"4310" /wd"4324" /wd"4702" /wd"4100" /wd"4819""'
  ) | Set-Content $marianCmake -NoNewline
}

# Patch 3: ssplit FindPCRE2.cmake — MSVC emits pcre2-8-static.lib
$findPcre = "3rd_party\ssplit-cpp\cmake\FindPCRE2.cmake"
Guarded-Patch $findPcre 'pcre2-8-static' {
  $content = Get-Content $findPcre -Raw
  $old = 'set(PCRE2_LIBRARIES ${CMAKE_BINARY_DIR}/${CMAKE_INSTALL_LIBDIR}/${CMAKE_STATIC_LIBRARY_PREFIX}pcre2-8${CMAKE_STATIC_LIBRARY_SUFFIX})'
  $new = @'
if(MSVC)
  set(PCRE2_LIBRARIES ${CMAKE_BINARY_DIR}/${CMAKE_INSTALL_LIBDIR}/${CMAKE_STATIC_LIBRARY_PREFIX}pcre2-8-static${CMAKE_STATIC_LIBRARY_SUFFIX})
else()
  set(PCRE2_LIBRARIES ${CMAKE_BINARY_DIR}/${CMAKE_INSTALL_LIBDIR}/${CMAKE_STATIC_LIBRARY_PREFIX}pcre2-8${CMAKE_STATIC_LIBRARY_SUFFIX})
endif()
'@
  $content = $content.Replace($old, $new)
  Set-Content -Path $findPcre -Value $content -NoNewline
}

# Patch 4: ssplit src/CMakeLists.txt — define PCRE2_STATIC for the dllimport shim
$ssplitCmake = "3rd_party\ssplit-cpp\src\CMakeLists.txt"
Guarded-Patch $ssplitCmake 'PCRE2_STATIC' {
  $content = Get-Content $ssplitCmake -Raw
  $old = 'add_library(ssplit STATIC ssplit/ssplit.cpp ssplit/regex.cpp)'
  $new = @"
add_library(ssplit STATIC ssplit/ssplit.cpp ssplit/regex.cpp)
if(WIN32 AND SSPLIT_USE_INTERNAL_PCRE2)
  target_compile_definitions(ssplit PRIVATE PCRE2_STATIC)
endif()
"@
  $content = $content.Replace($old, $new)
  Set-Content -Path $ssplitCmake -Value $content -NoNewline
}

# Patch 5 (no source edit): portable cmake + policy + flags passed on CLI below.

# ---- 3) Configure + build ----
if (-not (Test-Path (Join-Path $BuildDir "CMakeCache.txt"))) {
  & $Cmake -S $SrcDir -B $BuildDir -G "Visual Studio 17 2022" -A x64 `
    -DCMAKE_CONFIGURATION_TYPES=Release `
    -DSSPLIT_USE_INTERNAL_PCRE2=ON `
    -DUSE_WASM_COMPATIBLE_BLAS=ON `
    -DUSE_WASM_COMPATIBLE_SOURCE=ON `
    -DUSE_MKL=OFF `
    -DCMAKE_POLICY_VERSION_MINIMUM=3.5 `
    -DCMAKE_CXX_FLAGS="/DWIN32 /D_WINDOWS /W3 /GR /EHsc /utf-8"
  if ($LASTEXITCODE -ne 0) { throw "cmake configure failed" }
}
& $Cmake --build $BuildDir --config Release --target bergamot-translator
if ($LASTEXITCODE -ne 0) { throw "cmake build failed" }

# ---- 4) Collect outputs ----
New-Item -ItemType Directory -Force -Path $NativeLib, $NativeInc | Out-Null
$rel = Join-Path $BuildDir "Release"
Copy-Item (Join-Path $rel "marian.lib") -Destination $NativeLib -Force
Copy-Item (Join-Path $rel "ssplit.lib") -Destination $NativeLib -Force
Copy-Item (Join-Path $BuildDir "src\translator\Release\bergamot-translator.lib") -Destination $NativeLib -Force

# Transitive static libs marian/bergamot reference (link order in build.rs):
# sentencepiece(+train), yaml-cpp, intgemm, onnx-sgemm (WASM-compatible BLAS),
# pcre2 (ssplit). marian.lib does not bundle these symbols.
Copy-Item (Join-Path $BuildDir "3rd_party\marian-dev\src\3rd_party\sentencepiece\src\Release\sentencepiece.lib") -Destination $NativeLib -Force
Copy-Item (Join-Path $BuildDir "3rd_party\marian-dev\src\3rd_party\sentencepiece\src\Release\sentencepiece_train.lib") -Destination $NativeLib -Force
Copy-Item (Join-Path $BuildDir "3rd_party\marian-dev\src\3rd_party\yaml-cpp\libyaml-cpp.dir\Release\libyaml-cpp.lib") -Destination $NativeLib -Force
Copy-Item (Join-Path $BuildDir "3rd_party\marian-dev\src\3rd_party\intgemm\Release\intgemm.lib") -Destination $NativeLib -Force
Copy-Item (Join-Path $BuildDir "3rd_party\marian-dev\src\3rd_party\onnxjs\src\wasm-ops\Release\onnx-sgemm.lib") -Destination $NativeLib -Force
Copy-Item (Join-Path $BuildDir "lib\pcre2-8-static.lib") -Destination $NativeLib -Force

# Headers for the bridge — mirror the layout src-tauri/native/include expects:
#   include/translator/                 (bergamot public headers)
#   include/3rd_party/marian-dev/src/   (marian: common/, marian.h, 3rd_party/CLI)
#   include/3rd_party/yaml-cpp/         (parser.h: #include "3rd_party/yaml-cpp/yaml.h")
#   include/3rd_party/ssplit-cpp/       (text_processor.h: #include "ssplit.h")
Copy-Item (Join-Path $SrcDir "src\translator") $NativeInc -Recurse -Force
Copy-Item (Join-Path $SrcDir "3rd_party\marian-dev\src") (Join-Path $NativeInc "3rd_party\marian-dev\src") -Recurse -Force
Copy-Item (Join-Path $SrcDir "3rd_party\marian-dev\src\3rd_party\yaml-cpp") (Join-Path $NativeInc "3rd_party\yaml-cpp") -Recurse -Force
Copy-Item (Join-Path $SrcDir "3rd_party\ssplit-cpp\src\ssplit") (Join-Path $NativeInc "3rd_party\ssplit-cpp") -Recurse -Force

# Patch 6: mirrored marian logging.h — MSVC binds the template's unqualified
# `Logger` at instantiation, when marian::bergamot::Logger is also in scope.
# Qualify the global typedef so the bridge compiles under cl.exe.
$loggingH = Join-Path $NativeInc "3rd_party\marian-dev\src\common\logging.h"
Guarded-Patch $loggingH '::Logger log = spdlog' {
  (Get-Content $loggingH -Raw).Replace(
    '  Logger log = spdlog::get(logger);',
    '  ::Logger log = spdlog::get(logger);'
  ) | Set-Content $loggingH -NoNewline
}

# ---- 5) Compile + archive the C ABI bridge ----
# Depends only on the mirrored headers; produces bergamot_bridge.lib.
$vcvars = "$env:ProgramFiles(x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
$bridgeCpp = Join-Path $RepoRoot "src-tauri\native\bergamot_bridge.cpp"
$bridgeObj = Join-Path $NativeLib "bergamot_bridge.obj"
$bridgeLib = Join-Path $NativeLib "bergamot_bridge.lib"
$incRoots = "/I`"$NativeInc`" /I`"$NativeInc\3rd_party\marian-dev\src`" /I`"$NativeInc\3rd_party\marian-dev\src\3rd_party`" /I`"$NativeInc\3rd_party\ssplit-cpp`" /I`"$NativeInc\3rd_party\yaml-cpp`""
$clCmd = "`"$vcvars`" >nul 2>&1 && cl /nologo /c /std:c++17 /O2 /EHsc /utf-8 /DWIN32 /D_WINDOWS /DUSE_SSE2 /DWASM_COMPATIBLE_SOURCE /DWASM /DENABLE_CACHE_STATS $incRoots /Fo`"$bridgeObj`" `"$bridgeCpp`""
cmd /c $clCmd
if ($LASTEXITCODE -ne 0) { throw "bridge compile failed" }
cmd /c "`"$vcvars`" >nul 2>&1 && lib /nologo /machine:x64 /out:`"$bridgeLib`" `"$bridgeObj`""
if ($LASTEXITCODE -ne 0) { throw "bridge archive failed" }

Pop-Location
Write-Host ""
Write-Host "Native build complete."
Write-Host "  libs:  $NativeLib"
Write-Host "  incl:  $NativeInc"
Write-Host "Next: cargo check (build.rs links these when present)."
