fn main() {
    // Track the repository CRT policy even when only the build environment changes.
    println!("cargo:rerun-if-env-changed=STATIC_VCRUNTIME");
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/icon.png");
    tauri_build::build();
}
