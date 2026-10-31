#!/bin/bash
# apply_all.sh
#
# Convenience wrapper that applies all three Datadog asset types in the
# correct order. Useful for CI / fresh-onboarding / "just push everything
# I have locally".
#
# Order matters:
#   1. metric_configs — percentile aggregations have to exist on the metric
#      side before any dashboard widget can query `p95:metric{...}`. If
#      this step runs after dashboards, queries will return
#      `missing_aggregation` errors until the configs catch up.
#   2. dashboards — POST/PUT the dashboard JSON. Ids are written back into
#      the local files so subsequent runs are idempotent updates.
#   3. monitors — same pattern as dashboards.
#
# Passes any flags it receives (`--dry-run`, `--site …`) through to each
# sub-script.

set -euo pipefail

dir="$(cd "$(dirname "$0")" && pwd)"

echo "== applying metric configs =="
"$dir/apply_metric_configs.sh" "$@"

echo
echo "== applying dashboards =="
"$dir/apply_dashboards.sh" "$@"

echo
echo "== applying monitors =="
"$dir/apply_monitors.sh" "$@"

echo
echo "All Datadog assets applied."
