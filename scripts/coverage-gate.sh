#!/usr/bin/env bash
set -euo pipefail

report="${1:-target/coverage/cobertura.xml}"
workspace_min="${RGLINT_COVERAGE_WORKSPACE_MIN:-60}"
rules_module_min="${RGLINT_COVERAGE_RULES_MODULE_MIN:-90}"

if [[ ! -f "$report" ]]; then
  printf 'coverage gate: report not found: %s\n' "$report" >&2
  exit 2
fi

python3 - "$report" "$workspace_min" "$rules_module_min" <<'PY'
import sys
import xml.etree.ElementTree as ET
from collections import defaultdict
from pathlib import PurePosixPath

report, workspace_min, rules_module_min = sys.argv[1:]
workspace_min = float(workspace_min)
rules_module_min = float(rules_module_min)

try:
    root = ET.parse(report).getroot()
except (ET.ParseError, OSError) as error:
    print(f"coverage gate: cannot parse {report}: {error}", file=sys.stderr)
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
        print(f"rglint-rules/{module}: {rate:.2f}% ({covered[module]}/{valid[module]})")
        if rate < rules_module_min:
            failures.append(f"rglint-rules/{module} is below {rules_module_min:.2f}%")

if failures:
    for failure in failures:
        print(f"coverage gate: FAIL: {failure}", file=sys.stderr)
    raise SystemExit(1)
print("coverage gate: PASS")
PY
