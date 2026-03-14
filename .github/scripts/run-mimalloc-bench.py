# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Build rtmalloc shared libs, run mimalloc-bench, and generate an mdbook page with inline uPlot charts.

Usage:
    uv run .github/scripts/run-mimalloc-bench.py --output book/src/benchmarks/mimalloc-bench.md
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys

ALLOCATORS = [
    {"id": "sys",        "color": "#888888"},
    {"id": "rt_nightly", "color": "#2ca02c"},
    {"id": "rt_percpu",  "color": "#98df8a"},
    {"id": "rt_std",     "color": "#9467bd"},
    {"id": "rt_nostd",   "color": "#d62728"},
    {"id": "mi",         "color": "#17becf"},
    {"id": "tc",         "color": "#ff7f0e"},
    {"id": "je",         "color": "#1f77b4"},
    {"id": "sn",         "color": "#e377c2"},
    {"id": "rp",         "color": "#bcbd22"},
]

ALLOC_BY_ID = {a["id"]: a for a in ALLOCATORS}


def run(args, **kwargs):
    print(f"$ {' '.join(args)}", flush=True)
    return subprocess.run(args, check=True, **kwargs)


def build_shared_libs():
    run(["cargo", "+nightly", "rustc", "-p", "rtmalloc",
         "--features", "c-abi,nightly", "--crate-type", "cdylib", "--profile", "fast"])
    shutil.copy("target/fast/librtmalloc.so", "librtmalloc_nightly.so")

    run(["cargo", "+stable", "rustc", "-p", "rtmalloc",
         "--features", "c-abi,std", "--crate-type", "cdylib", "--profile", "fast"])
    shutil.copy("target/fast/librtmalloc.so", "librtmalloc_std.so")


def setup_mimalloc_bench():
    if not os.path.isdir("mimalloc-bench"):
        run(["git", "clone", "--depth", "1",
             "https://github.com/daanx/mimalloc-bench.git"])
    run(["bash", "./build-bench-env.sh", "bench", "mi", "je", "sn", "tc"],
        cwd="mimalloc-bench")


def run_mimalloc_bench():
    """Run mimalloc-bench with rtmalloc variants. Returns path to benchres.csv."""
    root = os.getcwd()
    bench_dir = os.path.join(root, "mimalloc-bench", "out", "bench")

    ext_path = os.path.join(root, "ext.txt")
    with open(ext_path, "w") as f:
        f.write(f"rt_nightly {root}/librtmalloc_nightly.so\n")
        f.write(f"rt_std {root}/librtmalloc_std.so\n")

    run(["bash", "../../bench.sh", f"--external={ext_path}",
         "mi", "je", "sn", "tc", "allt"],
        cwd=bench_dir)

    csv_path = os.path.join(root, "benchres.csv")
    shutil.copy(os.path.join(bench_dir, "benchres.csv"), csv_path)
    return csv_path


def parse_elapsed(s):
    """Parse elapsed time string (e.g. '1:23.45' or '83.45') into seconds."""
    s = s.strip()
    m = re.match(r"(?:(\d+):)?(\d+(?:\.\d+)?)", s)
    if not m:
        return None
    minutes = int(m.group(1)) if m.group(1) else 0
    seconds = float(m.group(2))
    return minutes * 60.0 + seconds


def parse_csv(path):
    """Parse benchres.csv → {benchmark: {allocator: elapsed_seconds}}."""
    groups = {}
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split()
            if len(parts) < 6:
                continue
            elapsed = parse_elapsed(parts[2])
            if elapsed is None:
                continue
            bench = parts[0]
            alloc = parts[1]
            groups.setdefault(bench, {})[alloc] = elapsed
    return groups


def generate_md(groups, output_path):
    """Write markdown with inline uPlot chart JSON per benchmark."""
    lines = ["# mimalloc-bench Charts\n",
             "Real-world program benchmarks from the main branch.\n"]

    for bench in sorted(groups.keys()):
        alloc_map = groups[bench]
        allocs = sorted(
            name for name in alloc_map
            if not name.endswith("_base")
        )
        if not allocs:
            continue

        datasets = []
        for a in allocs:
            datasets.append({
                "label": a,
                "color": ALLOC_BY_ID.get(a, {}).get("color", "#aaaaaa"),
                "data": [round(alloc_map[a], 3)],
            })

        chart = {
            "title": bench,
            "labels": [bench],
            "datasets": datasets,
            "axes": {"x": "Benchmark", "y": "Time (s)"},
        }

        lines.append("```uplot\n" + json.dumps(chart, indent=2) + "\n```\n")

    os.makedirs(os.path.dirname(os.path.abspath(output_path)), exist_ok=True)
    with open(output_path, "w") as f:
        f.write("\n".join(lines))

    print(f"Wrote {len(groups)} mimalloc-bench charts to {output_path}")


def main():
    parser = argparse.ArgumentParser(description="Run mimalloc-bench and generate chart markdown")
    parser.add_argument("--output", required=True, help="Output markdown file path")
    parser.add_argument("--skip-run", action="store_true",
                        help="Skip running benchmarks, just parse existing benchres.csv")
    parser.add_argument("--csv", help="Path to existing benchres.csv (implies --skip-run)")
    args = parser.parse_args()

    if args.csv:
        csv_path = args.csv
    elif args.skip_run:
        csv_path = "benchres.csv"
    else:
        build_shared_libs()
        setup_mimalloc_bench()
        csv_path = run_mimalloc_bench()

    groups = parse_csv(csv_path)
    if not groups:
        print("Warning: no mimalloc-bench results found", file=sys.stderr)
        with open(args.output, "w") as f:
            f.write("# mimalloc-bench Charts\n\n*No benchmark data available.*\n")
        return

    generate_md(groups, args.output)


if __name__ == "__main__":
    main()
