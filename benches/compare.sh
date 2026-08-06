#!/usr/bin/env sh
# Compare Criterion's current estimates with benches/baseline.json.
#
# Usage:
#   benches/compare.sh              # compare target/criterion/**/new
#   benches/compare.sh --update     # deliberately re-pin the baseline
#
# CRITERION_DIR can point at another Criterion output directory. The parser is
# kept in the standard library so the regression gate has no extra workspace
# dependency beyond the benchmark itself.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)
baseline="$script_dir/baseline.json"
criterion_dir=${CRITERION_DIR:-"$repo_dir/crates/rglint/target/criterion"}
mode=${1:-compare}

if [ "$mode" != "--update" ] && [ "$mode" != "compare" ]; then
    printf '%s\n' "usage: $0 [--update]" >&2
    exit 2
fi

python3 - "$baseline" "$criterion_dir" "$mode" <<'PY'
import json
import pathlib
import sys

baseline_path = pathlib.Path(sys.argv[1])
criterion_dir = pathlib.Path(sys.argv[2])
mode = sys.argv[3]

def estimates(root):
    values = {}
    if not root.is_dir():
        return values
    for path in root.rglob("estimates.json"):
        if path.parent.name != "new":
            continue
        relative = path.parent.parent.relative_to(root)
        key = "/".join(relative.parts)
        data = json.loads(path.read_text())
        values[key] = float(data["median"]["point_estimate"])
    return values

current = estimates(criterion_dir)
if mode == "--update":
    if not current:
        print(f"no Criterion estimates found under {criterion_dir}", file=sys.stderr)
        sys.exit(2)
    payload = {
        "format_version": 1,
        "tolerance": 0.10,
        "benchmarks": {
            key: {"median_ns": value} for key, value in sorted(current.items())
        },
    }
    baseline_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    print(f"updated {baseline_path} with {len(current)} benchmark(s)")
    sys.exit(0)

baseline = json.loads(baseline_path.read_text())
tolerance = float(baseline.get("tolerance", 0.10))
expected = baseline["benchmarks"]
missing = sorted(set(expected) - set(current))
failures = []
for key, entry in expected.items():
    if key not in current:
        continue
    old = float(entry["median_ns"])
    new = current[key]
    if old <= 0:
        failures.append(f"{key}: baseline median must be positive")
        continue
    change = (new - old) / old
    if change > tolerance:
        failures.append(
            f"{key}: {old:.0f} ns -> {new:.0f} ns ({change:+.1%}, limit +{tolerance:.0%})"
        )

if missing:
    print("missing benchmark(s); run benches/compare.sh --update deliberately:", file=sys.stderr)
    for key in missing:
        print(f"  {key}", file=sys.stderr)
if failures:
    print("benchmark regressions:", file=sys.stderr)
    for failure in failures:
        print(f"  {failure}", file=sys.stderr)
if missing or failures:
    sys.exit(1)
print(f"benchmark comparison passed ({len(expected)} benchmark(s), tolerance +{tolerance:.0%})")
PY
