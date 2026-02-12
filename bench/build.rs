use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(has_google_tcmalloc)");
    println!("cargo::rustc-check-cfg=cfg(has_rstcmalloc_percpu)");

    let ws_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // =========================================================================
    // Build rstcmalloc staticlibs with the `fast` profile:
    //   - nightly (#[thread_local] thread cache): --features nightly,ffi,testing
    //   - std     (std::thread_local! cache):     --features std,ffi,testing
    //   - nostd   (central cache only):           --features ffi,testing
    //   - percpu  (per-CPU rseq, Linux only):     --features percpu,ffi,testing
    // =========================================================================

    build_variant(&cargo, &ws_root, &out_dir, "nightly,ffi,testing", "rstcmalloc_nightly");
    build_variant(&cargo, &ws_root, &out_dir, "std,ffi,testing", "rstcmalloc_std");
    build_variant(&cargo, &ws_root, &out_dir, "ffi,testing", "rstcmalloc_nostd");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=rstcmalloc_nightly");
    println!("cargo:rustc-link-lib=static=rstcmalloc_std");
    println!("cargo:rustc-link-lib=static=rstcmalloc_nostd");

    // Per-CPU variant — only on Linux x86_64 (requires rseq)
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        build_variant(&cargo, &ws_root, &out_dir, "percpu,ffi,testing", "rstcmalloc_percpu");
        println!("cargo:rustc-link-lib=static=rstcmalloc_percpu");
        println!("cargo:rustc-cfg=has_rstcmalloc_percpu");
    }

    // The `std` variant's staticlib bundles a copy of libstd, which conflicts
    // with the sysroot's libstd that the bench binary also links.  Tell the
    // linker to tolerate the duplicate symbols (they are identical).
    #[cfg(windows)]
    println!("cargo:rustc-link-arg=/FORCE:MULTIPLE");
    #[cfg(not(windows))]
    println!("cargo:rustc-link-arg=-Wl,--allow-multiple-definition");

    // Windows: VirtualAlloc/VirtualFree live in kernel32
    #[cfg(windows)]
    println!("cargo:rustc-link-lib=dylib=kernel32");

    // Rerun if rstcmalloc source changes
    println!("cargo:rerun-if-changed=../src");
    println!("cargo:rerun-if-changed=../Cargo.toml");

    // =========================================================================
    // Google tcmalloc (optional, if vendor build exists)
    // =========================================================================

    let lib_dir = ws_root
        .join("target")
        .join("vendor")
        .join("gperftools-build")
        .join("Release");

    // Rerun if the vendor lib appears/changes so we pick it up
    println!("cargo:rerun-if-changed={}", lib_dir.join("tcmalloc_minimal.lib").display());

    if lib_dir.join("tcmalloc_minimal.lib").exists() {
        println!("cargo:rustc-cfg=has_google_tcmalloc");
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        println!("cargo:rustc-link-lib=static=tcmalloc_minimal");
        println!("cargo:rustc-link-lib=static=common");
        println!("cargo:rustc-link-lib=static=low_level_alloc");
    }
}

fn build_variant(cargo: &str, ws_root: &Path, out_dir: &Path, features: &str, lib_name: &str) {
    let target_dir = out_dir.join(format!("{lib_name}-build"));

    let status = Command::new(cargo)
        .arg("rustc")
        .arg("--manifest-path")
        .arg(ws_root.join("Cargo.toml"))
        .arg("-p")
        .arg("rstcmalloc")
        .arg("--profile")
        .arg("fast")
        .arg("--features")
        .arg(features)
        .arg("--crate-type")
        .arg("staticlib")
        .arg("--target-dir")
        .arg(&target_dir)
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn cargo for {lib_name}: {e}"));

    assert!(status.success(), "{lib_name} build failed");

    // Copy the staticlib to OUT_DIR with the variant name.
    // MSVC produces `rstcmalloc.lib`, GNU produces `librstcmalloc.a`.
    let fast_dir = target_dir.join("fast");
    let msvc_src = fast_dir.join("rstcmalloc.lib");
    let gnu_src = fast_dir.join("librstcmalloc.a");

    if msvc_src.exists() {
        std::fs::copy(&msvc_src, out_dir.join(format!("{lib_name}.lib")))
            .expect("failed to copy staticlib");
    } else if gnu_src.exists() {
        std::fs::copy(&gnu_src, out_dir.join(format!("lib{lib_name}.a")))
            .expect("failed to copy staticlib");
    } else {
        panic!("staticlib not found in {}", fast_dir.display());
    }
}
