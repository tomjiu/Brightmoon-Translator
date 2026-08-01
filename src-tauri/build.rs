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
}
