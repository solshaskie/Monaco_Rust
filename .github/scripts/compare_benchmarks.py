#!/usr/bin/env python3
"""
Compare benchmark results against a baseline.
Fail if any metric regresses by >20%.

Usage:
    python3 compare_benchmarks.py current.txt baseline.txt
"""

import re
import sys

REGRESSION_THRESHOLD = 1.20  # 20% slower = fail


def parse_benchmarks(path):
    """Extract metrics from benchmark output."""
    metrics = {}
    with open(path, "r") as f:
        for line in f:
            # Match patterns like:
            # "Small file insert: 45.23 µs/op"
            # "Undo/redo cycle: 12.50 µs/cycle"
            m = re.match(r"(.+?):\s+([0-9.]+)\s+(µs|op|ns|ms|cycle|call)/.*", line)
            if m:
                name = m.group(1).strip()
                value = float(m.group(2))
                metrics[name] = value
    return metrics


def compare(current, baseline):
    regressions = []
    for name, base_val in baseline.items():
        if name not in current:
            print(f"MISSING  {name}: no longer present in current run")
            regressions.append(name)
            continue

        cur_val = current[name]
        ratio = cur_val / base_val if base_val > 0 else 1.0

        status = "PASS"
        if ratio > REGRESSION_THRESHOLD:
            status = "FAIL"
            regressions.append(name)
        elif ratio > 1.05:
            status = "WARN"

        pct = (ratio - 1.0) * 100
        print(f"{status:4s}  {name}: baseline={base_val:.2f} current={cur_val:.2f} ({pct:+.1f}%)")

    for name in current:
        if name not in baseline:
            print(f"NEW    {name}: {current[name]:.2f} (new metric)")

    return regressions


def main():
    if len(sys.argv) < 3:
        print("Usage: compare_benchmarks.py current.txt baseline.txt")
        sys.exit(1)

    current = parse_benchmarks(sys.argv[1])
    baseline = parse_benchmarks(sys.argv[2])

    if not current or not baseline:
        print("Could not parse benchmark results.")
        sys.exit(1)

    print("Benchmark Comparison")
    print("=" * 60)
    regressions = compare(current, baseline)

    print()
    if regressions:
        print(f"FAILED: {len(regressions)} metric(s) regressed >20%")
        sys.exit(1)
    else:
        print("All metrics within threshold.")
        sys.exit(0)


if __name__ == "__main__":
    main()
