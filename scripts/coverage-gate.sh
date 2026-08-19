#!/usr/bin/env bash
set -euo pipefail

report="${1:-target/coverage/cobertura.xml}"
baseline="${2:-scripts/coverage-baseline.json}"
workspace_min="${RGLINT_COVERAGE_WORKSPACE_MIN:-60}"
rules_module_min="${RGLINT_COVERAGE_RULES_MODULE_MIN:-90}"

if [[ ! -f "$report" ]]; then
  printf 'coverage gate: report not found: %s\n' "$report" >&2
  exit 2
fi

if [[ ! -f "$baseline" ]]; then
  printf 'coverage gate: baseline not found: %s\n' "$baseline" >&2
  exit 2
fi

python3 - "$report" "$baseline" "$workspace_min" "$rules_module_min" <<'PY'
import json
import sys
import xml.etree.ElementTree as ET
from collections import defaultdict
from pathlib import PurePosixPath

report, baseline_path, workspace_min, rules_module_min = sys.argv[1:]
workspace_min = float(workspace_min)
rules_module_min = float(rules_module_min)

try:
    root = ET.parse(report).getroot()
except (ET.ParseError, OSError) as error:
    print(f"coverage gate: cannot parse {report}: {error}", file=sys.stderr)
    raise SystemExit(2)

try:
    with open(baseline_path, encoding="utf-8") as baseline_file:
        baselines = json.load(baseline_file)
except (json.JSONDecodeError, OSError) as error:
    print(f"coverage gate: cannot parse {baseline_path}: {error}", file=sys.stderr)
    raise SystemExit(2)

if not isinstance(baselines, dict):
    print("coverage gate: baseline root must be an object", file=sys.stderr)
    raise SystemExit(2)

for module, entry in baselines.items():
    valid_entry = (
        isinstance(module, str)
        and isinstance(entry, dict)
        and isinstance(entry.get("covered"), int)
        and not isinstance(entry.get("covered"), bool)
        and isinstance(entry.get("total"), int)
        and not isinstance(entry.get("total"), bool)
        and 0 <= entry["covered"] <= entry["total"]
        and entry["total"] > 0
        and isinstance(entry.get("reason"), str)
        and bool(entry["reason"].strip())
        and isinstance(entry.get("collector_exempt", False), bool)
    )
    if not valid_entry:
        print(f"coverage gate: invalid baseline entry for {module!r}", file=sys.stderr)
        raise SystemExit(2)

def percent(rate):
    return float(rate) * 100.0

root_rate = root.attrib.get("line-rate")
if root_rate is None:
    print("coverage gate: cobertura root is missing line-rate", file=sys.stderr)
    raise SystemExit(2)

print(f"workspace line coverage: {percent(root_rate):.2f}% (minimum {workspace_min:.2f}%)")
failures = []
if percent(root_rate) < workspace_min:
    failures.append(f"workspace coverage is below {workspace_min:.2f}%")

# Tarpaulin has emitted both package-oriented and class-oriented Cobertura
# layouts over time. Derive rules-module rates from source filenames and line
# hit counts so the check remains meaningful in either layout.
covered = defaultdict(int)
valid = defaultdict(int)
for cls in root.findall(".//class"):
    filename = cls.attrib.get("filename", "").replace("\\", "/")
    marker = "crates/rglint-rules/src/"
    if marker not in filename:
        continue
    module = filename.split(marker, 1)[1]
    if not module.endswith(".rs"):
        continue
    module = str(PurePosixPath(module))
    for line in cls.findall("./lines/line"):
        hits = line.attrib.get("hits")
        if hits is None:
            continue
        valid[module] += 1
        if int(hits) > 0:
            covered[module] += 1

if not valid:
    failures.append("no rglint-rules source modules were found in the coverage report")
else:
    for module in sorted(valid):
        rate = 100.0 * covered[module] / valid[module]
        entry = baselines.get(module)
        if entry is None:
            print(
                f"rglint-rules/{module}: {rate:.2f}% "
                f"({covered[module]}/{valid[module]}, minimum {rules_module_min:.2f}%)"
            )
            if rate < rules_module_min:
                failures.append(
                    f"rglint-rules/{module} is below {rules_module_min:.2f}%"
                )
            continue

        baseline_rate = 100.0 * entry["covered"] / entry["total"]
        if entry.get("collector_exempt", False):
            print(
                f"rglint-rules/{module}: collector exemption "
                f"({covered[module]}/{valid[module]} lines; pinned total {entry['total']})"
            )
            if valid[module] != entry["total"] or covered[module] != entry["covered"]:
                failures.append(
                    f"rglint-rules/{module} collector exemption changed; review and update the baseline"
                )
            continue

        print(
            f"rglint-rules/{module}: {rate:.2f}% "
            f"({covered[module]}/{valid[module]}, ratchet {baseline_rate:.2f}% "
            f"and {entry['covered']} covered lines; target {rules_module_min:.2f}%)"
        )
        if rate + 1e-9 < baseline_rate or covered[module] < entry["covered"]:
            failures.append(
                f"rglint-rules/{module} regressed below its reviewed coverage ratchet"
            )

    missing_modules = sorted(set(baselines) - set(valid))
    for module in missing_modules:
        failures.append(f"baseline module rglint-rules/{module} is missing from the coverage report")

if failures:
    for failure in failures:
        print(f"coverage gate: FAIL: {failure}", file=sys.stderr)
    raise SystemExit(1)
print("coverage gate: PASS")
PY
