#!/usr/bin/env python3
"""Parse mimalloc-bench benchres.csv into Markdown comparison tables and/or JSON for gh-pages tracking.

Usage:
    # PR comparison (base vs head):
    python parse-mimalloc-bench.py --csv benchres.csv --base-prefix _base --output-comment comment.md

    # Dashboard JSON (for bench-track):
    python parse-mimalloc-bench.py --csv benchres.csv --output-json results.json

    # Charts:
    python parse-mimalloc-bench.py --csv benchres.csv --output-charts charts/

    # All:
    python parse-mimalloc-bench.py --csv benchres.csv --base-prefix _base \
        --output-comment comment.md --output-json results.json --output-charts charts/
"""

import argparse
import json
import os
import re
import sys

try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    import numpy as np
    HAS_MATPLOTLIB = True
except ImportError:
    HAS_MATPLOTLIB = False

# Allocators tracked on the dashboard.
# None means track all allocators found in the CSV.
TRACKED_ALLOCATORS = None

# Threshold for flagging changes in PR comments.
CHANGE_THRESHOLD = 0.05  # 5%

# Colors for charts.
ALLOCATOR_COLORS = {
    "rt_nightly": "#2ca02c",
    "rt_std": "#9467bd",
    "mimalloc": "#17becf",
    "system": "#888888",
}


def parse_elapsed(s):
    """Parse elapsed time string (e.g., '1:23.45' or '83.45') into seconds."""
    s = s.strip()
    m = re.match(r"(?:(\d+):)?(\d+(?:\.\d+)?)", s)
    if not m:
        return None
    minutes = int(m.group(1)) if m.group(1) else 0
    seconds = float(m.group(2))
    return minutes * 60.0 + seconds


def parse_csv(path):
    """Parse benchres.csv into list of dicts."""
    rows = []
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
            rows.append({
                "benchmark": parts[0],
                "allocator": parts[1],
                "elapsed": elapsed,
                "rss_kb": int(parts[3]) if parts[3].isdigit() else 0,
                "user": float(parts[4]) if parts[4].replace(".", "").isdigit() else 0.0,
                "sys": float(parts[5]) if parts[5].replace(".", "").isdigit() else 0.0,
            })
    return rows


def group_by_benchmark(rows):
    """Group rows by benchmark name -> {allocator: row}."""
    groups = {}
    for r in rows:
        groups.setdefault(r["benchmark"], {})[r["allocator"]] = r
    return groups


def generate_comment(groups, base_suffix, output_path):
    """Generate a Markdown PR comparison comment."""
    benchmarks = sorted(groups.keys())

    # Find head/base allocator pairs
    head_allocs = set()
    for alloc_map in groups.values():
        for name in alloc_map:
            if not name.endswith(base_suffix):
                head_allocs.add(name)

    # Only compare allocators that have both head and base
    compared = sorted(
        name for name in head_allocs
        if any((name + base_suffix) in groups[b] for b in benchmarks)
    )

    # Also include non-rtmalloc allocators for reference
    reference_allocs = sorted(
        name for name in head_allocs
        if name not in compared and not name.endswith(base_suffix)
    )

    all_display = compared + reference_allocs

    lines = []
    lines.append("## mimalloc-bench Results\n")
    lines.append("> Comparing PR (head) vs main (base) across all standard benchmarks.\n")
    lines.append("")

    # Header
    cols = ["Benchmark"]
    for alloc in all_display:
        if alloc in compared:
            cols.append(f"{alloc} (head)")
            cols.append(f"{alloc} (base)")
            cols.append("Δ%")
        else:
            cols.append(alloc)
    lines.append("| " + " | ".join(cols) + " |")
    lines.append("| " + " | ".join(["---"] * len(cols)) + " |")

    regressions = []

    for bench in benchmarks:
        alloc_map = groups[bench]
        row = [f"**{bench}**"]

        for alloc in all_display:
            if alloc in compared:
                head = alloc_map.get(alloc)
                base = alloc_map.get(alloc + base_suffix)
                head_val = f"{head['elapsed']:.2f}s" if head else "—"
                base_val = f"{base['elapsed']:.2f}s" if base else "—"
                if head and base and base["elapsed"] > 0:
                    delta = (head["elapsed"] - base["elapsed"]) / base["elapsed"]
                    sign = "+" if delta > 0 else ""
                    emoji = ""
                    if delta > CHANGE_THRESHOLD:
                        emoji = " ⚠️"
                        regressions.append(f"{bench}/{alloc}: {sign}{delta:.1%}")
                    elif delta < -CHANGE_THRESHOLD:
                        emoji = " ✅"
                    delta_str = f"{sign}{delta:.1%}{emoji}"
                else:
                    delta_str = "—"
                row.extend([head_val, base_val, delta_str])
            else:
                entry = alloc_map.get(alloc)
                row.append(f"{entry['elapsed']:.2f}s" if entry else "—")

        lines.append("| " + " | ".join(row) + " |")

    lines.append("")

    if regressions:
        lines.append(f"**{len(regressions)} regression(s) detected (>{CHANGE_THRESHOLD:.0%}):**")
        for r in regressions:
            lines.append(f"- {r}")
        lines.append("")

    lines.append(
        "*Times are wall-clock seconds (lower is better). "
        "RSS shown in full results artifact.*"
    )

    with open(output_path, "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"Wrote comment to {output_path}")


def generate_json(groups, output_path):
    """Generate JSON for benchmark-action/github-action-benchmark."""
    entries = []
    for bench, alloc_map in sorted(groups.items()):
        for alloc_name, row in sorted(alloc_map.items()):
            if TRACKED_ALLOCATORS is not None and alloc_name not in TRACKED_ALLOCATORS:
                continue
            entries.append({
                "name": f"{bench}/{alloc_name}",
                "unit": "seconds",
                "value": round(row["elapsed"], 4),
            })
    with open(output_path, "w") as f:
        json.dump(entries, f, indent=2)
    print(f"Wrote {len(entries)} entries to {output_path}")


def generate_bmf_json(groups, output_path, base_suffix=None):
    """Generate Bencher Metric Format (BMF) JSON for bencher.dev."""
    bmf = {}
    for bench, alloc_map in sorted(groups.items()):
        for alloc_name, row in sorted(alloc_map.items()):
            if TRACKED_ALLOCATORS is not None and alloc_name not in TRACKED_ALLOCATORS:
                continue
            if base_suffix and alloc_name.endswith(base_suffix):
                continue
            bmf[f"{bench}/{alloc_name}"] = {"elapsed": {"value": round(row["elapsed"], 4)}}
    with open(output_path, "w") as f:
        json.dump(bmf, f, indent=2)
    print(f"Wrote {len(bmf)} BMF entries to {output_path}")


def generate_charts(groups, output_dir):
    """Generate SVG bar charts comparing allocators per benchmark."""
    if not HAS_MATPLOTLIB:
        print("matplotlib not available, skipping charts", file=sys.stderr)
        return

    os.makedirs(output_dir, exist_ok=True)

    benchmarks = sorted(groups.keys())
    # Collect all allocators (skip base variants)
    all_allocs = set()
    for alloc_map in groups.values():
        for name in alloc_map:
            if not name.endswith("_base"):
                all_allocs.add(name)
    allocs = sorted(all_allocs)

    for bench in benchmarks:
        alloc_map = groups[bench]
        present = [a for a in allocs if a in alloc_map]
        if not present:
            continue

        values = [alloc_map[a]["elapsed"] for a in present]
        colors = [ALLOCATOR_COLORS.get(a, "#aaaaaa") for a in present]

        fig, ax = plt.subplots(figsize=(max(6, len(present) * 0.8), 4))
        x = np.arange(len(present))
        ax.bar(x, values, color=colors, width=0.6)
        ax.set_xticks(x)
        ax.set_xticklabels(present, rotation=45, ha="right", fontsize=8)
        ax.set_ylabel("Elapsed (seconds)")
        ax.set_title(bench)
        fig.tight_layout()
        fig.savefig(os.path.join(output_dir, f"{bench}.svg"), format="svg")
        plt.close(fig)

    # Generate index.html
    chart_files = sorted(f for f in os.listdir(output_dir) if f.endswith(".svg"))
    html = ["<!DOCTYPE html><html><head><title>mimalloc-bench Charts</title></head><body>"]
    html.append("<h1>mimalloc-bench Charts</h1>")
    for cf in chart_files:
        name = cf.replace(".svg", "")
        html.append(f"<h2>{name}</h2><img src='{cf}' style='max-width:800px'>")
    html.append("</body></html>")
    with open(os.path.join(output_dir, "index.html"), "w") as f:
        f.write("\n".join(html))

    print(f"Wrote {len(chart_files)} charts to {output_dir}/")


def main():
    parser = argparse.ArgumentParser(description="Parse mimalloc-bench results")
    parser.add_argument("--csv", required=True, help="Path to benchres.csv")
    parser.add_argument("--base-prefix", default="_base",
                        help="Suffix identifying base allocator names (default: _base)")
    parser.add_argument("--output-comment", help="Write Markdown comparison to this file")
    parser.add_argument("--output-json", help="Write dashboard JSON to this file (github-action-benchmark format)")
    parser.add_argument("--output-bmf", help="Write Bencher Metric Format JSON to this file (bencher.dev)")
    parser.add_argument("--output-charts", help="Write SVG charts to this directory")
    args = parser.parse_args()

    rows = parse_csv(args.csv)
    if not rows:
        print(f"No results found in {args.csv}", file=sys.stderr)
        sys.exit(1)

    groups = group_by_benchmark(rows)
    print(f"Parsed {len(rows)} results across {len(groups)} benchmarks")

    if args.output_comment:
        generate_comment(groups, args.base_prefix, args.output_comment)

    if args.output_json:
        generate_json(groups, args.output_json)

    if args.output_bmf:
        generate_bmf_json(groups, args.output_bmf, base_suffix=args.base_prefix)

    if args.output_charts:
        generate_charts(groups, args.output_charts)


if __name__ == "__main__":
    main()
