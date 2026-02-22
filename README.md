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
