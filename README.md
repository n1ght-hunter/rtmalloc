# rtmalloc: Rust Thread-caching malloc

## About
rtmalloc is a ground up new malloc written in rust based heavily on tcmalloc. 
The main reasons for this are:
- Having a malloc native to rust requireing no c build tools to compile
- Learning experience of writing a malloc
- Experimenting with new ideas in malloc design following the ideas of pgo(profile guided optimization). tcmalloc already does somthing simliar.
- Wanted a simple malloc for my own language project that I can easily modify and experiment with.

## Features
- Thread local caching of small allocations using a per thread arena design
- Experimental cpu cache aware allocation design using a per cpu arena design with rseq.
- 3 Part design following tcmalloc with frontend(per-thread/cpu), central(global) and backend(page heap) allocators

## Roadmap
- [x] Implement a basic malloc with a single global arena
- [x] Implement a per thread arena design for small allocations
- [ ] Implement a per cpu arena design for small allocations using rseq experimental
- [ ] Benchmark and make sure rtmalloc nightly is within 1% the speed of tcmalloc 
- [ ] Impl profiling with an output to have custom class sizes for better cache performance
- [ ] Find a way to run Miri without explicit `MIRIFLAGS` (currently needs `-Zmiri-ignore-leaks -Zmiri-permissive-provenance` because caching allocators hold memory in free lists and use integer↔pointer casts internally)

## Usage

Add rtmalloc as a dependency and set it as the global allocator:

```rust
use rtmalloc::RtMalloc;

#[global_allocator]
static GLOBAL: RtMalloc = RtMalloc;
```

For best performance on nightly Rust, enable the `nightly` feature for `#[thread_local]` support:

```toml
[dependencies]
rtmalloc = { path = ".", features = ["nightly"] }
```

### Configuration

All allocator tuning is done through a single TOML file. By default rtmalloc uses `default_classes.toml` in the crate root. To use a custom config, set the `RTMALLOC_CLASSES` env var at build time:

```bash
RTMALLOC_CLASSES=my_config.toml cargo build
```

The config has two sections — `[config]` for global knobs and `[[class]]` for size class definitions. All `[config]` fields are optional and default to sane values:

```toml
[config]
page_size = 8192           # must be power of 2, >= 4096
thread_cache_size = 33554432   # 32 MiB total thread cache budget
max_transfer_slots = 64        # batches cached per size class
max_pages = 128                # page heap bucket count

# Size classes — listed smallest to largest, must be 8-byte aligned.
# Each class can optionally specify pages and batch_size.
[[class]]
size = 8

[[class]]
size = 16

# ... up to 63 classes
```

Alternatively, use the simple shorthand format for auto-tuned classes:

```toml
classes = [8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192]
```

<details>
<summary><strong>Profiling & Optimising Size Classes</strong></summary>

rtmalloc ships with a built-in allocation histogram that records every allocation size at runtime. You can use it to generate a custom size class config tuned to your workload.

#### 1. Enable the histogram

```toml
[dependencies]
rtmalloc = { path = ".", features = ["alloc-histogram", "nightly"] }
```

#### 2. Run your workload, then print the report

```rust
// At shutdown or after a representative run:
rtmalloc::histogram::print_report();
```

This prints a bucket-by-bucket breakdown plus a suggested class layout with waste stats and a ready-to-use TOML snippet.

#### 3. Export a config file directly

```rust
let toml = rtmalloc::histogram::export_toml(64, 0.125);
std::fs::write("profile_classes.toml", toml).unwrap();
```

#### 4. Rebuild with the profiled config

```bash
RTMALLOC_CLASSES=profile_classes.toml cargo build --release
```

The `optimal_layout` algorithm greedily merges adjacent size buckets to minimise internal fragmentation while staying under a waste-per-class threshold (`max_waste_pct`). This is the same PGO-style feedback loop that tcmalloc uses internally.

</details>

<details>
<summary><strong>Runtime Stats</strong></summary>

Enable the `stats` feature to collect allocation/deallocation counters with zero contention (per-thread atomics):

```toml
[dependencies]
rtmalloc = { path = ".", features = ["stats", "nightly"] }
```

Stats are recorded via the `stat_inc!` / `stat_add!` macros inside the allocator. When the feature is disabled, these compile to nothing.

</details>

## Benchmarks

Benchmarks are still in progress, but the goal is to have rtmalloc be within 1% the speed of tcmalloc on a variety of workloads.
you can run the benchmarks with `cargo bench -p rtmalloc_bench` 
if you wish for tcmalloc to be included in the benchmarks you can build it with `cargo +nightly -Zscript scripts/build_tcmalloc.rs`

# Contributing
Contributions are welcome! Please open an issue or submit a pull request.

## Achnowledgements
- tcmalloc for the design and inspiration of this malloc(https://github.com/gperftools/gperftools)

## License
Licensed under either of
- MIT License (LICENSE-MIT or http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
