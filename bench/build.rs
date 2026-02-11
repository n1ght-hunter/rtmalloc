fn main() {
    println!("cargo::rustc-check-cfg=cfg(has_google_tcmalloc)");

    // The workspace root is one level up from this crate.
    let ws_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();

    let lib_dir = ws_root
        .join("target")
        .join("vendor")
        .join("gperftools-build")
        .join("Release");

    if lib_dir.join("tcmalloc_minimal.lib").exists() {
        println!("cargo:rustc-cfg=has_google_tcmalloc");
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        println!("cargo:rustc-link-lib=static=tcmalloc_minimal");
        println!("cargo:rustc-link-lib=static=common");
        println!("cargo:rustc-link-lib=static=low_level_alloc");
    }
}
