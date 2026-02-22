#!/usr/bin/env python3
"""Extract Criterion benchmark results into JSON (for dashboard), Markdown (for PR comments),
and bar chart SVGs (for visual comparison).

Usage:
    # Dashboard JSON only (for bench-track.yml):
    python extract-criterion.py --head target/criterion --output-json bench-results.json

    # PR comparison comment (for bench-pr.yml):
    python extract-criterion.py --base target/criterion-base --head target/criterion --output-comment bench-comment.md

    # Charts only:
    python extract-criterion.py --head target/criterion --output-charts bench-charts

    # All outputs:
    python extract-criterion.py --base target/criterion-base --head target/criterion \
        --output-json bench-results.json --output-comment bench-comment.md --output-charts bench-charts
"""

import argparse
import json
import os
import sys

try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    import matplotlib.ticker as ticker
    import numpy as np
    HAS_MATPLOTLIB = True
except ImportError:
    HAS_MATPLOTLIB = False

# Only track rtmalloc variants in the dashboard (not system/mimalloc).
TRACKED_ALLOCATORS = {"rt_nightly", "rt_std", "rt_nostd", "rt_percpu"}

# Threshold for flagging regressions/improvements in PR comments.
CHANGE_THRESHOLD = 0.05  # 5%

# Canonical allocator ordering (matches KNOWN in alloc_bench.rs).
ALLOCATOR_ORDER = [
    "system",
    "rt_nightly",
    "rt_percpu",
    "rt_std",
    "rt_nostd",
    "mimalloc",
    "google_tc",
    "jemalloc",
    "snmalloc",
    "rpmalloc",
]

# Hex colors matching svg_color_for() in alloc_bench.rs.
ALLOCATOR_COLORS = {
    "system":       "#888888",
    "rt_nightly": "#2ca02c",
    "rt_percpu":  "#98df8a",
    "rt_std":     "#9467bd",
    "rt_nostd":   "#d62728",
    "mimalloc":     "#17becf",
    "google_tc":    "#ff7f0e",
    "jemalloc":     "#1f77b4",
    "snmalloc":     "#e377c2",
    "rpmalloc":     "#bcbd22",
}

# Display names for chart labels.
ALLOCATOR_LABELS = {
    "system":       "system",
    "rt_nightly": "rt (nightly)",
    "rt_percpu":  "rt (percpu)",
    "rt_std":     "rt (std)",
    "rt_nostd":   "rt (nostd)",
    "mimalloc":     "mimalloc",
    "google_tc":    "tcmalloc",
    "jemalloc":     "jemalloc",
    "snmalloc":     "snmalloc",
    "rpmalloc":     "rpmalloc",
}


def scan_criterion_dir(criterion_path):
    """Walk a criterion output directory and collect median estimates.

    Returns dict mapping benchmark name -> median nanoseconds.
    Benchmark names look like "group/allocator/param" or "group/allocator".
    """
    results = {}
    for root, dirs, files in os.walk(criterion_path):
        if "estimates.json" not in files:
            continue
        # Only look at the "new" subdirectory (criterion stores base/new)
        if os.path.basename(root) != "new":
            continue

        estimates_path = os.path.join(root, "estimates.json")
        try:
            with open(estimates_path) as f:
                data = json.load(f)
        except (json.JSONDecodeError, OSError):
            continue

        median_ns = data.get("median", {}).get("point_estimate")
        if median_ns is None:
            continue

        # Build the benchmark name from the path relative to criterion_path.
        # e.g. criterion_path/single_alloc_dealloc/rt_nightly/8/new/estimates.json
        #   -> single_alloc_dealloc/rt_nightly/8
        rel = os.path.relpath(root, criterion_path)
        # Remove trailing "/new"
        parts = rel.replace("\\", "/").split("/")
        if parts and parts[-1] == "new":
            parts = parts[:-1]
        name = "/".join(parts)

        results[name] = median_ns

    return results


def extract_allocator(name):
    """Extract the allocator name from a benchmark name like 'group/allocator/param'."""
    parts = name.split("/")
    if len(parts) >= 2:
        return parts[1]
    return parts[0]


def extract_group(name):
    """Extract the group name from 'group/allocator/param'."""
    return name.split("/")[0]


def extract_param(name):
    """Extract the param from 'group/allocator/param', or None if absent."""
    parts = name.split("/")
    if len(parts) >= 3:
        return parts[2]
    return None


def is_tracked(name):
    """Check if a benchmark name is for a tracked rtmalloc variant."""
    return extract_allocator(name) in TRACKED_ALLOCATORS


def format_ns(ns):
    """Format nanoseconds into a human-readable string."""
    if ns < 1_000:
        return f"{ns:.1f} ns"
    elif ns < 1_000_000:
        return f"{ns / 1_000:.2f} us"
    elif ns < 1_000_000_000:
        return f"{ns / 1_000_000:.2f} ms"
    else:
        return f"{ns / 1_000_000_000:.2f} s"


def to_benchmark_json(results):
    """Convert results to github-action-benchmark's customSmallerIsBetter format.

    Only includes tracked rtmalloc allocator variants.
    """
    entries = []
    for name in sorted(results.keys()):
        if not is_tracked(name):
            continue
        entries.append({
            "name": name,
            "unit": "ns",
            "value": round(results[name], 2),
        })
    return entries


def generate_comparison_comment(base_results, head_results):
    """Generate a Markdown comparison comment for a PR."""
    # Collect all benchmark names present in either run
    all_names = sorted(set(base_results.keys()) | set(head_results.keys()))

    improved = []
    regressed = []
    unchanged = []
    new_benchmarks = []
    removed_benchmarks = []

    for name in all_names:
        if name not in base_results:
            new_benchmarks.append(name)
            continue
        if name not in head_results:
            removed_benchmarks.append(name)
            continue

        base_ns = base_results[name]
        head_ns = head_results[name]

        if base_ns == 0:
            unchanged.append((name, base_ns, head_ns, 0.0))
            continue

        change = (head_ns - base_ns) / base_ns

        entry = (name, base_ns, head_ns, change)
        if change < -CHANGE_THRESHOLD:
            improved.append(entry)
        elif change > CHANGE_THRESHOLD:
            regressed.append(entry)
        else:
            unchanged.append(entry)

    # Build the Markdown
    lines = ["## Benchmark Comparison\n"]

    # Separate rtmalloc-only stats
    rt_improved = [e for e in improved if is_tracked(e[0])]
    rt_regressed = [e for e in regressed if is_tracked(e[0])]
    rt_unchanged = [e for e in unchanged if is_tracked(e[0])]

    lines.append(
        f"**rtmalloc variants:** "
        f"{'✅ ' + str(len(rt_improved)) + ' improved, ' if rt_improved else ''}"
        f"{'⚠️ ' + str(len(rt_regressed)) + ' regressed, ' if rt_regressed else ''}"
        f"{len(rt_unchanged)} unchanged "
        f"(>{int(CHANGE_THRESHOLD * 100)}% threshold)\n"
    )

    if rt_regressed:
        lines.append("> ⚠️ **Performance regressions detected in rtmalloc.** Please review below.\n")
    elif rt_improved:
        lines.append("> ✅ **Performance improvements detected!**\n")
    else:
        lines.append("> No significant changes in rtmalloc variants.\n")

    # Group benchmarks by group name (first path component)
    groups = {}
    all_entries = improved + regressed + unchanged
    for entry in all_entries:
        name = entry[0]
        group = name.split("/")[0]
        groups.setdefault(group, []).append(entry)

    lines.append("<details><summary>Full results</summary>\n")

    for group in sorted(groups.keys()):
        entries = sorted(groups[group], key=lambda e: e[0])
        lines.append(f"### {group}\n")
        lines.append("| Allocator | Param | Base | Head | Change |")
        lines.append("|-----------|-------|-----:|-----:|-------:|")

        for name, base_ns, head_ns, change in entries:
            parts = name.split("/")
            allocator = parts[1] if len(parts) >= 2 else parts[0]
            param = parts[2] if len(parts) >= 3 else "-"

            change_str = f"{change:+.1%}"
            if change > CHANGE_THRESHOLD:
                change_str += " ⚠️"
            elif change < -CHANGE_THRESHOLD:
                change_str += " ✅"

            lines.append(
                f"| {allocator} | {param} "
                f"| {format_ns(base_ns)} | {format_ns(head_ns)} "
                f"| {change_str} |"
            )

        lines.append("")

    if new_benchmarks:
        lines.append("### New benchmarks\n")
        for name in new_benchmarks:
            if name in head_results:
                lines.append(f"- **{name}**: {format_ns(head_results[name])}")
        lines.append("")

    if removed_benchmarks:
        lines.append("### Removed benchmarks\n")
        for name in removed_benchmarks:
            lines.append(f"- ~~{name}~~")
        lines.append("")

    lines.append("</details>")

    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Chart generation
# ---------------------------------------------------------------------------

def _auto_scale_ns(ns_values):
    """Choose the best time unit for a set of nanosecond values.

    Returns (divisor, unit_label).
    """
    if not ns_values:
        return (1.0, "ns")
    max_val = max(ns_values)
    if max_val >= 1_000_000_000:
        return (1_000_000_000, "s")
    elif max_val >= 1_000_000:
        return (1_000_000, "ms")
    elif max_val >= 1_000:
        return (1_000, "\u00b5s")
    else:
        return (1.0, "ns")


def _structure_by_group(results):
    """Reshape flat results into {group: {param: {allocator: ns}}}.

    Only includes allocators present in ALLOCATOR_COLORS.
    For groups without params, the param key is None.
    """
    groups = {}
    for name, ns in results.items():
        group = extract_group(name)
        alloc = extract_allocator(name)
        param = extract_param(name)

        if alloc not in ALLOCATOR_COLORS:
            continue

        groups.setdefault(group, {}).setdefault(param, {})[alloc] = ns

    return groups


def _param_sort_key(p):
    """Sort params numerically if possible, then lexicographically."""
    if p is None:
        return (0, "")
    try:
        return (0, int(p))
    except ValueError:
        return (1, p)


def _x_label_for_group(group_name):
    """Contextual x-axis label based on group name."""
    if group_name == "thread_scalability":
        return "Threads"
    if group_name in ("single_alloc_dealloc", "batch_1000", "churn"):
        return "Allocation size (bytes)"
    return "Parameter"


def _generate_parameterized_chart(group_name, params, allocators, param_data, output_dir):
    """Clustered bar chart: x-axis = params, bars grouped by allocator."""
    n_allocs = len(allocators)
    n_params = len(params)

    all_ns = []
    for p in params:
        for a in allocators:
            v = param_data.get(p, {}).get(a)
            if v is not None:
                all_ns.append(v)

    divisor, unit = _auto_scale_ns(all_ns)

    fig_width = min(20, max(8, n_params * (n_allocs * 0.35 + 0.5) + 2))
    fig, ax = plt.subplots(figsize=(fig_width, 5.5))

    x = np.arange(n_params)
    bar_width = 0.8 / n_allocs

    for i, alloc in enumerate(allocators):
        values = []
        for p in params:
            v = param_data.get(p, {}).get(alloc)
            values.append(v / divisor if v is not None else 0)
        offset = (i - n_allocs / 2 + 0.5) * bar_width
        ax.bar(
            x + offset, values, bar_width,
            label=ALLOCATOR_LABELS.get(alloc, alloc),
            color=ALLOCATOR_COLORS.get(alloc, "#999999"),
            edgecolor="white",
            linewidth=0.3,
        )

    param_labels = [str(p) if p is not None else group_name for p in params]
    ax.set_xticks(x)
    ax.set_xticklabels(param_labels)
    ax.set_xlabel(_x_label_for_group(group_name))
    ax.set_ylabel(f"Time ({unit})")
    ax.set_title(group_name.replace("_", " ").title(), fontweight="bold", fontsize=13)

    if all_ns:
        ratio = max(all_ns) / max(min(all_ns), 1e-9)
        if ratio > 10:
            ax.set_yscale("log")
            ax.yaxis.set_major_formatter(ticker.ScalarFormatter())
            ax.yaxis.get_major_formatter().set_scientific(False)

    ax.legend(
        loc="upper left",
        bbox_to_anchor=(1.01, 1),
        fontsize=8,
        frameon=True,
        borderaxespad=0,
    )
    ax.grid(axis="y", alpha=0.3, linestyle="--")
    ax.set_axisbelow(True)

    fig.tight_layout()
    svg_path = os.path.join(output_dir, f"{group_name}.svg")
    fig.savefig(svg_path, format="svg", bbox_inches="tight",
                metadata={"Creator": "rtmalloc-bench"})
    plt.close(fig)
    return svg_path


def _generate_simple_chart(group_name, allocators, alloc_data, output_dir):
    """Single bar per allocator (no param axis)."""
    all_ns = [alloc_data.get(a, 0) for a in allocators]
    divisor, unit = _auto_scale_ns([v for v in all_ns if v > 0])

    fig_width = max(6, len(allocators) * 0.9 + 2)
    fig, ax = plt.subplots(figsize=(fig_width, 5))

    x = np.arange(len(allocators))
    values = [alloc_data.get(a, 0) / divisor for a in allocators]
    colors = [ALLOCATOR_COLORS.get(a, "#999999") for a in allocators]
    labels = [ALLOCATOR_LABELS.get(a, a) for a in allocators]

    bars = ax.bar(x, values, 0.6, color=colors, edgecolor="white", linewidth=0.5)

    for bar, val in zip(bars, values):
        if val > 0:
            ax.text(
                bar.get_x() + bar.get_width() / 2, bar.get_height(),
                f"{val:.1f}",
                ha="center", va="bottom", fontsize=7, color="#333333",
            )

    ax.set_xticks(x)
    ax.set_xticklabels(labels, rotation=30, ha="right", fontsize=9)
    ax.set_ylabel(f"Time ({unit})")
    ax.set_title(group_name.replace("_", " ").title(), fontweight="bold", fontsize=13)

    ax.grid(axis="y", alpha=0.3, linestyle="--")
    ax.set_axisbelow(True)

    fig.tight_layout()
    svg_path = os.path.join(output_dir, f"{group_name}.svg")
    fig.savefig(svg_path, format="svg", bbox_inches="tight",
                metadata={"Creator": "rtmalloc-bench"})
    plt.close(fig)
    return svg_path


def _generate_index_html(svg_files, output_dir):
    """Generate an index.html that displays all chart SVGs."""
    html = [
        "<!DOCTYPE html>",
        "<html><head>",
        '<meta charset="utf-8">',
        "<title>rtmalloc Benchmark Charts</title>",
        "<style>",
        "  body { font-family: system-ui, sans-serif; max-width: 1200px;"
        " margin: 0 auto; padding: 20px; background: #fafafa; }",
        "  h1 { color: #333; }",
        "  .chart { background: white; border: 1px solid #ddd;"
        " border-radius: 8px; padding: 16px; margin: 24px 0; }",
        "  .chart img { width: 100%; height: auto; }",
        "  .timestamp { color: #999; font-size: 0.85em; }",
        "</style>",
        "</head><body>",
        "<h1>rtmalloc Benchmark Charts</h1>",
        '<p class="timestamp">Generated from latest main branch push</p>',
    ]
    for svg in sorted(svg_files):
        name = svg.replace(".svg", "").replace("_", " ").title()
        html.append(
            f'<div class="chart"><h2>{name}</h2>'
            f'<img src="{svg}" alt="{name}"></div>'
        )
    html.append("</body></html>")

    with open(os.path.join(output_dir, "index.html"), "w") as f:
        f.write("\n".join(html))


def generate_charts(results, output_dir):
    """Generate grouped bar chart SVGs, one per benchmark group.

    Returns list of generated file basenames.
    """
    if not HAS_MATPLOTLIB:
        print("Warning: matplotlib not available, skipping chart generation",
              file=sys.stderr)
        return []

    os.makedirs(output_dir, exist_ok=True)
    groups = _structure_by_group(results)
    generated = []

    for group_name in sorted(groups.keys()):
        param_data = groups[group_name]

        all_allocators = set()
        for alloc_map in param_data.values():
            all_allocators.update(alloc_map.keys())
        allocators = [a for a in ALLOCATOR_ORDER if a in all_allocators]

        if not allocators:
            continue

        params = sorted(param_data.keys(), key=_param_sort_key)
        has_params = len(params) > 1 or (len(params) == 1 and params[0] is not None)

        if has_params:
            _generate_parameterized_chart(
                group_name, params, allocators, param_data, output_dir
            )
        else:
            the_param = params[0]
            _generate_simple_chart(
                group_name, allocators, param_data[the_param], output_dir
            )

        generated.append(f"{group_name}.svg")

    _generate_index_html(generated, output_dir)
    generated.append("index.html")

    return generated


def main():
    parser = argparse.ArgumentParser(description="Extract Criterion benchmark results")
    parser.add_argument("--base", help="Path to base criterion directory (for comparison)")
    parser.add_argument("--head", required=True, help="Path to head criterion directory")
    parser.add_argument("--output-json", help="Output path for dashboard JSON")
    parser.add_argument("--output-comment", help="Output path for PR comparison Markdown")
    parser.add_argument("--output-charts", help="Output directory for comparison chart SVGs")
    args = parser.parse_args()

    head_results = scan_criterion_dir(args.head)
    if not head_results:
        print(f"Warning: no benchmark results found in {args.head}", file=sys.stderr)

    if args.output_json:
        entries = to_benchmark_json(head_results)
        with open(args.output_json, "w") as f:
            json.dump(entries, f, indent=2)
        print(f"Wrote {len(entries)} entries to {args.output_json}")

    if args.output_charts:
        chart_files = generate_charts(head_results, args.output_charts)
        print(f"Generated {len(chart_files)} chart files in {args.output_charts}")

    if args.output_comment:
        if not args.base:
            print("Error: --base is required when using --output-comment", file=sys.stderr)
            sys.exit(1)
        base_results = scan_criterion_dir(args.base)
        if not base_results:
            print(f"Warning: no benchmark results found in {args.base}", file=sys.stderr)
        comment = generate_comparison_comment(base_results, head_results)
        with open(args.output_comment, "w") as f:
            f.write(comment)
        print(f"Wrote comparison comment to {args.output_comment}")


if __name__ == "__main__":
    main()
