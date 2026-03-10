use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;

struct ClassDef {
    size: usize,
    pages: usize,
    batch_size: usize,
}

fn auto_pages(size: usize, page_size: usize) -> usize {
    if size <= page_size {
        1
    } else if size <= page_size * 4 {
        (size * 8).div_ceil(page_size)
    } else {
        (size * 2).div_ceil(page_size)
    }
}

fn auto_batch(size: usize, page_size: usize) -> usize {
    if size <= 1024 {
        32
    } else if size <= 4096 {
        (65536 / size).max(2)
    } else {
        (page_size / size).max(2)
    }
}

fn auto_class(size: usize, page_size: usize) -> ClassDef {
    ClassDef {
        size,
        pages: auto_pages(size, page_size),
        batch_size: auto_batch(size, page_size),
    }
}

#[derive(Deserialize, Default)]
struct ConfigSection {
    page_size: Option<usize>,
    thread_cache_size: Option<usize>,
    min_per_thread_cache: Option<usize>,
    steal_amount: Option<usize>,
    max_free_list_length: Option<u32>,
    max_overages: Option<u32>,
    max_transfer_slots: Option<usize>,
    max_pages: Option<usize>,
}

#[derive(Deserialize, Default)]
struct Config {
    #[serde(default)]
    config: ConfigSection,
    #[serde(default)]
    classes: Vec<usize>,
    #[serde(default, rename = "class")]
    class_full: Vec<ClassFull>,
}

#[derive(Deserialize)]
struct ClassFull {
    size: usize,
    pages: Option<usize>,
    batch_size: Option<usize>,
}

struct ResolvedConfig {
    page_size: usize,
    page_shift: u32,
    thread_cache_size: usize,
    min_per_thread_cache: usize,
    steal_amount: usize,
    max_free_list_length: u32,
    max_overages: u32,
    max_transfer_slots: usize,
    max_pages: usize,
}

fn resolve_config(cfg: &ConfigSection) -> ResolvedConfig {
    let page_size = cfg.page_size.unwrap_or(8192);
    assert!(
        page_size > 0 && page_size.is_power_of_two(),
        "page_size ({}) must be a power of 2",
        page_size
    );
    assert!(
        page_size >= 4096,
        "page_size ({}) must be >= 4096",
        page_size
    );

    let thread_cache_size = cfg.thread_cache_size.unwrap_or(32 * 1024 * 1024);
    let min_per_thread_cache = cfg.min_per_thread_cache.unwrap_or(512 * 1024);
    let steal_amount = cfg.steal_amount.unwrap_or(64 * 1024);
    let max_free_list_length = cfg.max_free_list_length.unwrap_or(8192);
    let max_overages = cfg.max_overages.unwrap_or(3);
    let max_transfer_slots = cfg.max_transfer_slots.unwrap_or(64);
    let max_pages = cfg.max_pages.unwrap_or(128);

    assert!(thread_cache_size > 0, "thread_cache_size must be > 0");
    assert!(min_per_thread_cache > 0, "min_per_thread_cache must be > 0");
    assert!(
        thread_cache_size >= min_per_thread_cache,
        "thread_cache_size ({}) must be >= min_per_thread_cache ({})",
        thread_cache_size,
        min_per_thread_cache
    );
    assert!(steal_amount > 0, "steal_amount must be > 0");
    assert!(max_free_list_length > 0, "max_free_list_length must be > 0");
    assert!(max_overages > 0, "max_overages must be > 0");
    assert!(max_transfer_slots > 0, "max_transfer_slots must be > 0");
    assert!(max_pages > 0, "max_pages must be > 0");

    ResolvedConfig {
        page_size,
        page_shift: page_size.trailing_zeros(),
        thread_cache_size,
        min_per_thread_cache,
        steal_amount,
        max_free_list_length,
        max_overages,
        max_transfer_slots,
        max_pages,
    }
}

fn parse_classes(config: &Config, page_size: usize) -> Vec<ClassDef> {
    if !config.classes.is_empty() && !config.class_full.is_empty() {
        panic!("RTMALLOC_CLASSES: use either `classes = [...]` or `[[class]]`, not both");
    }

    let defs: Vec<ClassDef> = if !config.classes.is_empty() {
        config
            .classes
            .iter()
            .map(|&s| auto_class(s, page_size))
            .collect()
    } else if !config.class_full.is_empty() {
        config
            .class_full
            .iter()
            .map(|c| ClassDef {
                size: c.size,
                pages: c.pages.unwrap_or_else(|| auto_pages(c.size, page_size)),
                batch_size: c
                    .batch_size
                    .unwrap_or_else(|| auto_batch(c.size, page_size)),
            })
            .collect()
    } else {
        panic!("RTMALLOC_CLASSES: config must contain `classes` or `[[class]]` entries");
    };

    validate_classes(&defs);
    defs
}

fn validate_classes(defs: &[ClassDef]) {
    assert!(
        !defs.is_empty(),
        "RTMALLOC_CLASSES: no size classes defined"
    );
    assert!(
        defs.len() < 64,
        "RTMALLOC_CLASSES: too many classes ({}, max 63)",
        defs.len()
    );
    for (i, d) in defs.iter().enumerate() {
        assert!(d.size > 0, "class {}: size must be > 0", i);
        assert!(
            d.size % 8 == 0,
            "class {}: size {} must be 8-byte aligned",
            i,
            d.size
        );
        assert!(d.pages > 0, "class {}: pages must be > 0", i);
        assert!(d.batch_size > 0, "class {}: batch_size must be > 0", i);
        if i > 0 {
            assert!(
                d.size > defs[i - 1].size,
                "class {}: size {} must be > previous size {}",
                i,
                d.size,
                defs[i - 1].size
            );
        }
    }
}

fn default_config_path() -> String {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    format!("{}/rtmalloc.toml", manifest_dir)
}

fn generate_config(cfg: &ResolvedConfig, out_path: &Path) {
    let code = format!(
        "// Auto-generated by build.rs. Do not edit.\n\n\
         pub const PAGE_SHIFT: usize = {};\n\
         pub const PAGE_SIZE: usize = {};\n\
         pub const OVERALL_THREAD_CACHE_SIZE: usize = {};\n\
         pub const MIN_PER_THREAD_CACHE_SIZE: usize = {};\n\
         pub const STEAL_AMOUNT: usize = {};\n\
         pub const MAX_DYNAMIC_FREE_LIST_LENGTH: u32 = {};\n\
         pub const MAX_OVERAGES: u32 = {};\n\
         pub const MAX_TRANSFER_SLOTS: usize = {};\n\
         pub const MAX_PAGES: usize = {};\n",
        cfg.page_shift,
        cfg.page_size,
        cfg.thread_cache_size,
        cfg.min_per_thread_cache,
        cfg.steal_amount,
        cfg.max_free_list_length,
        cfg.max_overages,
        cfg.max_transfer_slots,
        cfg.max_pages,
    );
    fs::write(out_path, code).expect("failed to write config_gen.rs");
}

fn generate_size_classes(defs: &[ClassDef], out_path: &Path) {
    let num_size_classes = defs.len() + 1;

    let mut code = String::from("// Auto-generated by build.rs. Do not edit.\n\n");

    code.push_str(&format!(
        "pub static SIZE_CLASSES: [SizeClassInfo; {num_size_classes}] = [\n\
         \x20   SizeClassInfo {{ size: 0, pages: 0, batch_size: 0 }}, // sentinel\n",
    ));
    for d in defs {
        code.push_str(&format!(
            "    SizeClassInfo {{ size: {}, pages: {}, batch_size: {} }},\n",
            d.size, d.pages, d.batch_size
        ));
    }
    code.push_str("];\n");

    fs::write(out_path, code).expect("failed to write size_class_gen.rs");
}

fn main() {
    println!("cargo:rerun-if-env-changed=RTMALLOC_CLASSES");

    let out_dir = env::var("OUT_DIR").unwrap();

    let config_path = env::var("RTMALLOC_CLASSES").unwrap_or_else(|_| default_config_path());
    println!("cargo:rerun-if-changed={}", config_path);
    let content = fs::read_to_string(&config_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", config_path, e));

    let config: Config = toml::from_str(&content).expect("failed to parse TOML config");

    let resolved = resolve_config(&config.config);
    let defs = parse_classes(&config, resolved.page_size);

    generate_config(&resolved, &Path::new(&out_dir).join("config_gen.rs"));
    generate_size_classes(&defs, &Path::new(&out_dir).join("size_class_gen.rs"));
}
