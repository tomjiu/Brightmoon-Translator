// Build scripts fail hard on errors by design; `expect` here is idiomatic.
#![allow(clippy::expect_used)]

fn main() {
    // Disable tauri-build's default manifest (it injects only into the bin via
    // cargo:rustc-link-arg-bins, so the lib unit-test exe gets none →
    // STATUS_ENTRYPOINT_NOT_FOUND 0xc0000139). Use a global cargo:rustc-link-arg
    // to inject the comctl32 v6 manifest into every target (bin / bin-test /
    // lib-test). icon + version info are still provided by tauri-build's
    // resource.lib.
    // SAFETY: new_without_app_manifest only sets app_manifest to None; icon/version unaffected.
    let attrs = tauri_build::Attributes::new()
        .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    tauri_build::try_build(attrs).expect("failed to run tauri build script");

    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:tests.manifest");
    println!("cargo:rerun-if-changed=tests.manifest");

    link_native_bergamot();
}

/// Link the native bergamot stack when it has been built.
///
/// `scripts/build-bergamot-native.ps1` produces these static libs under
/// `src-tauri/native/lib/` (gitignored). When they are absent we emit nothing
/// and set no cfg, so `cargo check`/tests still compile (the offline engine
/// degrades gracefully). The link order matters: each lib may reference
/// symbols defined by the libs that follow it.
#[cfg(target_os = "windows")]
fn link_native_bergamot() {
    let native = std::path::Path::new("native/lib");
    let libs = [
        "bergamot_bridge",
        "bergamot-translator",
        "marian",
        "sentencepiece_train",
        "sentencepiece",
        "libyaml-cpp",
        "intgemm",
        "ssplit",
        "pcre2-8-static",
        "onnx-sgemm",
    ];

    if native.join("bergamot_bridge.lib").exists() {
        for lib in &libs {
            println!("cargo:rustc-link-lib=static={lib}");
        }
        println!("cargo:rustc-link-search=native={}", native.display());
        // marian uses SHGetFolderPathW (shell32) and PathMatchSpecW (shlwapi).
        println!("cargo:rustc-link-lib=dylib=shell32");
        println!("cargo:rustc-link-lib=dylib=shlwapi");
        println!("cargo:rustc-cfg=bergamot_native");
        println!("cargo:rustc-check-cfg=cfg(bergamot_native)");
        println!("cargo:rerun-if-changed=native/lib");
    }
}

#[cfg(not(target_os = "windows"))]
fn link_native_bergamot() {}
