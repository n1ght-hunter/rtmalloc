#!/usr/bin/env python3
"""Extract Criterion benchmark results into JSON (for dashboard) and Markdown (for PR comments).

Usage:
    # Dashboard JSON only (for bench-track.yml):
    python extract-criterion.py --head target/criterion --output-json bench-results.json

    # PR comparison comment (for bench-pr.yml):
    python extract-criterion.py --base target/criterion-base --head target/criterion --output-comment bench-comment.md

    # Both:
    python extract-criterion.py --base target/criterion-base --head target/criterion \
        --output-json bench-results.json --output-comment bench-comment.md
"""

import argparse
import json
import os
import sys

# Only track rstcmalloc variants in the dashboard (not system/mimalloc).
TRACKED_ALLOCATORS = {"rstc_nightly", "rstc_std", "rstc_nostd", "rstc_percpu"}

# Threshold for flagging regressions/improvements in PR comments.
CHANGE_THRESHOLD = 0.05  # 5%


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
        # e.g. criterion_path/single_alloc_dealloc/rstc_nightly/8/new/estimates.json
        #   -> single_alloc_dealloc/rstc_nightly/8
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


def is_tracked(name):
    """Check if a benchmark name is for a tracked rstcmalloc variant."""
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

    Only includes tracked rstcmalloc allocator variants.
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

    # Separate rstcmalloc-only stats
    rstc_improved = [e for e in improved if is_tracked(e[0])]
    rstc_regressed = [e for e in regressed if is_tracked(e[0])]
    rstc_unchanged = [e for e in unchanged if is_tracked(e[0])]

    lines.append(
        f"**rstcmalloc variants:** "
        f"{'✅ ' + str(len(rstc_improved)) + ' improved, ' if rstc_improved else ''}"
        f"{'⚠️ ' + str(len(rstc_regressed)) + ' regressed, ' if rstc_regressed else ''}"
        f"{len(rstc_unchanged)} unchanged "
        f"(>{int(CHANGE_THRESHOLD * 100)}% threshold)\n"
    )

    if rstc_regressed:
        lines.append("> ⚠️ **Performance regressions detected in rstcmalloc.** Please review below.\n")
    elif rstc_improved:
        lines.append("> ✅ **Performance improvements detected!**\n")
    else:
        lines.append("> No significant changes in rstcmalloc variants.\n")

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


def main():
    parser = argparse.ArgumentParser(description="Extract Criterion benchmark results")
    parser.add_argument("--base", help="Path to base criterion directory (for comparison)")
    parser.add_argument("--head", required=True, help="Path to head criterion directory")
    parser.add_argument("--output-json", help="Output path for dashboard JSON")
    parser.add_argument("--output-comment", help="Output path for PR comparison Markdown")
    args = parser.parse_args()

    head_results = scan_criterion_dir(args.head)
    if not head_results:
        print(f"Warning: no benchmark results found in {args.head}", file=sys.stderr)

    if args.output_json:
        entries = to_benchmark_json(head_results)
        with open(args.output_json, "w") as f:
            json.dump(entries, f, indent=2)
        print(f"Wrote {len(entries)} entries to {args.output_json}")

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
