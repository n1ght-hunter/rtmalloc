#!/usr/bin/env -S cargo +nightly -Zscript
---
[dependencies]
---

//! Downloads and builds Google's tcmalloc (gperftools) from source.
//!
//! Usage: cargo +nightly -Zscript scripts/build_tcmalloc.rs

use std::path::PathBuf;
use std::process::Command;

fn run(cmd: &mut Command) {
    println!(">>> {:?}", cmd);
    let status = cmd.status().expect("failed to execute command");
    if !status.success() {
        panic!("command failed with {status}");
    }
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // When run as a cargo script, CARGO_MANIFEST_DIR may not point to our repo.
    // Use the script's own location instead.
    let root = if root.join("Cargo.toml").exists() && root.join("src").exists() {
        root
    } else {
        // Fallback: assume script is in <root>/scripts/
        std::env::current_dir().unwrap()
    };

    let vendor = root.join("target").join("vendor");
    let source = vendor.join("gperftools");
    let build = vendor.join("gperftools-build");
    let install = vendor.join("gperftools-install");

    let repo = "https://github.com/gperftools/gperftools.git";
    let tag = "gperftools-2.18";

    println!("=== Building Google tcmalloc (gperftools) ===");
    println!("  Root:    {}", root.display());
    println!("  Source:  {}", source.display());
    println!("  Build:   {}", build.display());
    println!("  Install: {}", install.display());
    println!();

    // Clone if needed
    if !source.exists() {
        println!(">>> Cloning gperftools ({tag})...");
        std::fs::create_dir_all(&vendor).expect("failed to create vendor dir");
        run(Command::new("git")
            .args(["clone", "--depth", "1", "--branch", tag, repo])
            .arg(&source));
    } else {
        println!(">>> gperftools source already present, skipping clone.");
    }

    // Configure with CMake
    println!(">>> Configuring with CMake...");
    std::fs::create_dir_all(&build).expect("failed to create build dir");
    run(Command::new("cmake")
        .arg("-S").arg(&source)
        .arg("-B").arg(&build)
        .args([
            "-DCMAKE_BUILD_TYPE=Release",
            &format!("-DCMAKE_INSTALL_PREFIX={}", install.display()),
            "-DBUILD_SHARED_LIBS=OFF",
            "-DBUILD_TESTING=OFF",
        ]));

    // Build
    println!(">>> Building...");
    run(Command::new("cmake")
        .args(["--build"])
        .arg(&build)
        .args(["--config", "Release", "--parallel"]));

    // Install
    println!(">>> Installing...");
    run(Command::new("cmake")
        .args(["--install"])
        .arg(&build)
        .args(["--config", "Release"]));

    println!();
    println!("=== Done ===");
    println!("Libraries in: {}", install.join("lib").display());

    // List output
    if let Ok(entries) = std::fs::read_dir(install.join("lib")) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.contains("tcmalloc") || name.contains("profiler") {
                println!("  {name}");
            }
        }
    }
}
