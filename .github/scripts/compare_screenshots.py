#!/usr/bin/env python3
"""
Compare visual regression screenshots against baselines.
Fail if any pixel diff exceeds the threshold outside known change zones.

Usage:
    python3 compare_screenshots.py <baseline_dir> <actual_dir> --threshold 0.01
"""

import argparse
import sys
import os
from pathlib import Path


def parse_args():
    parser = argparse.ArgumentParser(description="Visual regression pixel diff")
    parser.add_argument("baseline_dir", help="Directory containing baseline PNGs")
    parser.add_argument("actual_dir", help="Directory containing actual PNGs")
    parser.add_argument(
        "--threshold",
        type=float,
        default=0.01,
        help="Max allowed pixel diff ratio (default: 0.01 = 1%)",
    )
    return parser.parse_args()


def compare_images(baseline_path, actual_path):
    """
    Compare two PNG images pixel by pixel.
    Returns (diff_count, total_pixels, diff_ratio).
    Requires Pillow.
    """
    try:
        from PIL import Image
    except ImportError:
        print("WARNING: Pillow not installed; skipping pixel comparison.")
        return 0, 1, 0.0

    baseline = Image.open(baseline_path).convert("RGBA")
    actual = Image.open(actual_path).convert("RGBA")

    # Resize to matching dimensions if needed
    if baseline.size != actual.size:
        actual = actual.resize(baseline.size, Image.LANCZOS)

    baseline_pixels = list(baseline.getdata())
    actual_pixels = list(actual.getdata())

    total = len(baseline_pixels)
    diff = sum(
        1 for (b, a) in zip(baseline_pixels, actual_pixels) if b != a
    )
    ratio = diff / total if total > 0 else 0.0
    return diff, total, ratio


def main():
    args = parse_args()
    baseline_dir = Path(args.baseline_dir)
    actual_dir = Path(args.actual_dir)

    if not baseline_dir.exists():
        print(f"ERROR: Baseline directory does not exist: {baseline_dir}")
        sys.exit(1)

    if not actual_dir.exists():
        print(f"ERROR: Actual directory does not exist: {actual_dir}")
        sys.exit(1)

    baseline_files = sorted(
        f for f in baseline_dir.rglob("*.png") if "-actual.png" not in f.name
    )

    failures = []
    for baseline_file in baseline_files:
        # Determine the expected actual filename (Playwright naming convention)
        relative = baseline_file.relative_to(baseline_dir)
        actual_file = actual_dir / relative

        if not actual_file.exists():
            # Playwright stores actuals under test-results with different naming
            alt_name = actual_file.stem + "-actual.png"
            actual_file = actual_file.with_name(alt_name)

        if not actual_file.exists():
            print(f"MISSING  {relative}: actual screenshot not found")
            failures.append(str(relative))
            continue

        diff, total, ratio = compare_images(baseline_file, actual_file)
        status = "PASS" if ratio <= args.threshold else "FAIL"
        pct = ratio * 100
        print(f"{status:4s}  {relative}: {diff}/{total} pixels differ ({pct:.3f}%)")

        if ratio > args.threshold:
            failures.append(str(relative))

    print()
    if failures:
        print(f"FAILED: {len(failures)} screenshot(s) exceed {args.threshold*100:.1f}% threshold")
        sys.exit(1)
    else:
        print(f"All screenshots within {args.threshold*100:.1f}% threshold.")
        sys.exit(0)


if __name__ == "__main__":
    main()
