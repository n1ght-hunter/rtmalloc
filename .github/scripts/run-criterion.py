# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Run criterion benchmarks and generate an mdbook page with inline uPlot charts.

Usage:
    uv run .github/scripts/run-criterion.py --output book/src/benchmarks/criterion.md
"""

import argparse
import json
import os
import subprocess
import sys

ALLOCATORS = [
    {"id": "system",     "label": "system",       "color": "#888888"},
    {"id": "rt_nightly", "label": "rt (nightly)",  "color": "#2ca02c"},
    {"id": "rt_percpu",  "label": "rt (percpu)",   "color": "#98df8a"},
    {"id": "rt_std",     "label": "rt (std)",      "color": "#9467bd"},
    {"id": "rt_nostd",   "label": "rt (nostd)",     "color": "#d62728"},
    {"id": "mimalloc",   "label": "mimalloc",      "color": "#17becf"},
    {"id": "google_tc",  "label": "tcmalloc",      "color": "#ff7f0e"},
    {"id": "jemalloc",   "label": "jemalloc",      "color": "#1f77b4"},
    {"id": "snmalloc",   "label": "snmalloc",      "color": "#e377c2"},
    {"id": "rpmalloc",   "label": "rpmalloc",      "color": "#bcbd22"},
]


ALLOC_BY_ID = {a["id"]: a for a in ALLOCATORS}

X_LABELS = {
    "thread_scalability": "Threads",
    "single_alloc_dealloc": "Allocation size (bytes)",
    "batch_5000": "Allocation size (bytes)",
    "churn": "Allocation size (bytes)",
}


def run_benchmarks():
    subprocess.run(
        ["cargo", "+nightly", "bench", "-p", "rtmalloc-bench",
         "--bench", "alloc_bench", "--", "--noplot"],
        check=True,
    )


def scan_criterion(criterion_path):
    """Walk criterion output, return {name: median_ns}."""
    results = {}
    for root, dirs, files in os.walk(criterion_path):
        if "estimates.json" not in files:
            continue
        if os.path.basename(root) != "new":
            continue

        with open(os.path.join(root, "estimates.json")) as f:
            data = json.load(f)

        median_ns = data.get("median", {}).get("point_estimate")
        if median_ns is None:
            continue

        rel = os.path.relpath(root, criterion_path)
        parts = rel.replace("\\", "/").split("/")
        if parts and parts[-1] == "new":
            parts = parts[:-1]

        results["/".join(parts)] = median_ns

    return results


def auto_scale(ns_values):
    """Choose best time unit. Returns (divisor, label)."""
    if not ns_values:
        return (1.0, "ns")
    m = max(ns_values)
    if m >= 1e9:
        return (1e9, "s")
    if m >= 1e6:
        return (1e6, "ms")
    if m >= 1e3:
        return (1e3, "\u00b5s")
    return (1.0, "ns")


def structure_by_group(results):
    """Reshape {name: ns} into {group: {param: {allocator: ns}}}."""
    groups = {}
    for name, ns in results.items():
        parts = name.split("/")
        group = parts[0]
        alloc = parts[1] if len(parts) >= 2 else parts[0]
        param = parts[2] if len(parts) >= 3 else None

        if alloc not in ALLOC_BY_ID:
            continue

        groups.setdefault(group, {}).setdefault(param, {})[alloc] = ns

    return groups


def param_sort_key(p):
    if p is None:
        return (0, "")
    try:
        return (0, int(p))
    except ValueError:
        return (1, p)


def generate_md(results, output_path):
    """Write markdown with inline uPlot chart JSON per benchmark group."""
    groups = structure_by_group(results)
    lines = ["# Criterion Benchmark Charts\n",
             "Latest allocator comparison from the main branch.\n"]

    for group_name in sorted(groups.keys()):
        param_data = groups[group_name]

        all_allocs = set()
        for alloc_map in param_data.values():
            all_allocs.update(alloc_map.keys())
        allocators = [a["id"] for a in ALLOCATORS if a["id"] in all_allocs]
        if not allocators:
            continue

        params = sorted(param_data.keys(), key=param_sort_key)

        all_ns = []
        for p in params:
            for a in allocators:
                v = param_data.get(p, {}).get(a)
                if v is not None:
                    all_ns.append(v)

        divisor, unit = auto_scale(all_ns)
        labels = [str(p) if p is not None else group_name for p in params]

        datasets = []
        for alloc in allocators:
            data = []
            for p in params:
                v = param_data.get(p, {}).get(alloc)
                data.append(round(v / divisor, 2) if v is not None else None)
            info = ALLOC_BY_ID.get(alloc, {})
            datasets.append({
                "label": info.get("label", alloc),
                "color": info.get("color", "#999999"),
                "data": data,
            })

        chart = {
            "title": group_name.replace("_", " ").title(),
            "labels": labels,
            "datasets": datasets,
            "axes": {
                "x": X_LABELS.get(group_name, "Parameter"),
                "y": f"Time ({unit})",
            },
        }

        lines.append("```uplot\n" + json.dumps(chart, indent=2) + "\n```\n")

    os.makedirs(os.path.dirname(os.path.abspath(output_path)), exist_ok=True)
    with open(output_path, "w") as f:
        f.write("\n".join(lines))

    print(f"Wrote criterion charts to {output_path}")


def main():
    parser = argparse.ArgumentParser(description="Run criterion benchmarks and generate chart markdown")
    parser.add_argument("--output", required=True, help="Output markdown file path")
    parser.add_argument("--skip-run", action="store_true",
                        help="Skip running benchmarks, just parse existing results")
    args = parser.parse_args()

    if not args.skip_run:
        run_benchmarks()

    results = scan_criterion("target/criterion")
    if not results:
        print("Warning: no criterion results found", file=sys.stderr)
        with open(args.output, "w") as f:
            f.write("# Criterion Benchmark Charts\n\n*No benchmark data available.*\n")
        return

    generate_md(results, args.output)


if __name__ == "__main__":
    main()
