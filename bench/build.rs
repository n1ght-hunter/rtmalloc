use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(has_google_tcmalloc)");
    println!("cargo::rustc-check-cfg=cfg(has_rtmalloc_percpu)");
    println!("cargo::rustc-check-cfg=cfg(has_jemalloc)");

    // jemalloc is available on non-MSVC targets (Cargo.toml uses target cfg)
    #[cfg(not(target_env = "msvc"))]
    println!("cargo:rustc-cfg=has_jemalloc");

    let ws_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // =========================================================================
    // Build rtmalloc staticlibs with the `fast` profile:
    //   - nightly (#[thread_local] thread cache): --features nightly,ffi,testing
    //   - std     (std::thread_local! cache):     --features std,ffi,testing
    //   - nostd   (central cache only):           --features ffi,testing
    //   - percpu  (per-CPU rseq, Linux only):     --features percpu,ffi,testing
    // =========================================================================

    build_variant(
        &cargo,
        &ws_root,
        &out_dir,
        "nightly,ffi,testing",
        "rtmalloc_nightly",
    );
    build_variant(
        &cargo,
        &ws_root,
        &out_dir,
        "std,ffi,testing",
        "rtmalloc_std",
    );
    build_variant(&cargo, &ws_root, &out_dir, "ffi,testing", "rtmalloc_nostd");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=rtmalloc_nightly");
    println!("cargo:rustc-link-lib=static=rtmalloc_std");
    println!("cargo:rustc-link-lib=static=rtmalloc_nostd");

    // Per-CPU variant — only on Linux x86_64 (requires rseq)
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        build_variant(
            &cargo,
            &ws_root,
            &out_dir,
            "percpu,ffi,testing",
            "rtmalloc_percpu",
        );
        println!("cargo:rustc-link-lib=static=rtmalloc_percpu");
        println!("cargo:rustc-cfg=has_rtmalloc_percpu");
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

    // Rerun if rtmalloc source changes
    println!("cargo:rerun-if-changed=../src");
    println!("cargo:rerun-if-changed=../Cargo.toml");

    // =========================================================================
    // Google tcmalloc (auto-build from source, optional)
    // =========================================================================

    let vendor = ws_root.join("target").join("vendor");
    let install_lib = vendor.join("gperftools-install").join("lib");
    let build_release = vendor.join("gperftools-build").join("Release");

    // Rerun if the vendor lib appears/changes so we pick it up
    println!("cargo:rerun-if-changed={}", install_lib.display());
    println!("cargo:rerun-if-changed={}", build_release.display());

    if try_build_google_tcmalloc(&ws_root) {
        println!("cargo:rustc-cfg=has_google_tcmalloc");

        // Link from whichever location has the library
        if lib_exists(&install_lib, "tcmalloc_minimal") {
            println!("cargo:rustc-link-search=native={}", install_lib.display());
        } else if lib_exists(&build_release, "tcmalloc_minimal") {
            println!("cargo:rustc-link-search=native={}", build_release.display());
        }

        println!("cargo:rustc-link-lib=static=tcmalloc_minimal");

        // Windows MSVC build also produces common.lib and low_level_alloc.lib
        #[cfg(windows)]
        {
            println!("cargo:rustc-link-lib=static=common");
            println!("cargo:rustc-link-lib=static=low_level_alloc");
        }

        // Linux: gperftools needs pthreads
        #[cfg(not(windows))]
        println!("cargo:rustc-link-lib=dylib=pthread");
    }
}

// =========================================================================
// tcmalloc auto-build helpers
// =========================================================================

/// Attempt to build Google tcmalloc (gperftools) from source using git + cmake.
/// Returns true if the library is available (either pre-built or freshly built).
fn try_build_google_tcmalloc(ws_root: &Path) -> bool {
    let vendor = ws_root.join("target").join("vendor");
    let source = vendor.join("gperftools");
    let build_dir = vendor.join("gperftools-build");
    let install_dir = vendor.join("gperftools-install");
    let install_lib = install_dir.join("lib");
    let build_release = build_dir.join("Release");

    // Already built? Skip.
    if lib_exists(&install_lib, "tcmalloc_minimal")
        || lib_exists(&build_release, "tcmalloc_minimal")
    {
        return true;
    }

    // Check for git
    if !tool_available("git") {
        println!("cargo:warning=git not found, skipping Google tcmalloc build");
        return false;
    }

    // Check for cmake
    if !tool_available("cmake") {
        println!("cargo:warning=cmake not found, skipping Google tcmalloc build");
        return false;
    }

    // Clone if needed
    if !source.exists() {
        println!("cargo:warning=Cloning gperftools (this only happens once)...");
        let _ = std::fs::create_dir_all(&vendor);
        let status = Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                "--branch",
                "gperftools-2.18",
                "https://github.com/gperftools/gperftools.git",
            ])
            .arg(&source)
            .status();
        match status {
            Ok(s) if s.success() => {}
            _ => {
                println!("cargo:warning=Failed to clone gperftools, skipping tcmalloc");
                return false;
            }
        }
    }

    // CMake configure
    println!("cargo:warning=Building Google tcmalloc from source (this only happens once)...");
    let _ = std::fs::create_dir_all(&build_dir);
    let status = Command::new("cmake")
        .arg("-S")
        .arg(&source)
        .arg("-B")
        .arg(&build_dir)
        .args([
            "-DCMAKE_BUILD_TYPE=Release",
            &format!("-DCMAKE_INSTALL_PREFIX={}", install_dir.display()),
            "-DBUILD_SHARED_LIBS=OFF",
            "-DBUILD_TESTING=OFF",
            // Use static CRT (/MT) to match snmalloc-sys
            "-DCMAKE_POLICY_DEFAULT_CMP0091=NEW",
            "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded",
            "-DCMAKE_C_FLAGS_RELEASE=/MT /O2 /Ob2 /DNDEBUG",
            "-DCMAKE_CXX_FLAGS_RELEASE=/MT /O2 /Ob2 /DNDEBUG",
        ])
        .status();
    if !matches!(status, Ok(s) if s.success()) {
        println!("cargo:warning=CMake configure failed for gperftools, skipping tcmalloc");
        return false;
    }

    // CMake build
    let status = Command::new("cmake")
        .arg("--build")
        .arg(&build_dir)
        .args(["--config", "Release", "--parallel"])
        .status();
    if !matches!(status, Ok(s) if s.success()) {
        println!("cargo:warning=CMake build failed for gperftools, skipping tcmalloc");
        return false;
    }

    // CMake install
    let status = Command::new("cmake")
        .arg("--install")
        .arg(&build_dir)
        .args(["--config", "Release"])
        .status();
    if !matches!(status, Ok(s) if s.success()) {
        println!("cargo:warning=CMake install failed for gperftools, skipping tcmalloc");
        return false;
    }

    // Verify the library appeared
    lib_exists(&install_lib, "tcmalloc_minimal") || lib_exists(&build_release, "tcmalloc_minimal")
}

/// Check if a tool (git, cmake) is available on PATH.
fn tool_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Check if a static library exists in the given directory.
/// Handles both MSVC (.lib) and GNU (.a) naming conventions.
fn lib_exists(dir: &Path, name: &str) -> bool {
    dir.join(format!("{name}.lib")).exists() || dir.join(format!("lib{name}.a")).exists()
}

// =========================================================================
// rtmalloc variant builder
// =========================================================================

fn build_variant(cargo: &str, ws_root: &Path, out_dir: &Path, features: &str, lib_name: &str) {
    let target_dir = out_dir.join(format!("{lib_name}-build"));

    let status = Command::new(cargo)
        .arg("rustc")
        .arg("--manifest-path")
        .arg(ws_root.join("Cargo.toml"))
        .arg("-p")
        .arg("rtmalloc")
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
    // MSVC produces `rtmalloc.lib`, GNU produces `librtmalloc.a`.
    let fast_dir = target_dir.join("fast");
    let msvc_src = fast_dir.join("rtmalloc.lib");
    let gnu_src = fast_dir.join("librtmalloc.a");

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
