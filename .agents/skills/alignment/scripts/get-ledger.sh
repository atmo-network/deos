#!/bin/bash
# Script to query, filter, and format alignment ledger entries.

show_help() {
  echo "Usage: get-ledger.sh [OPTIONS]"
  echo "Query, filter, and summarize DEOS alignment ledgers."
  echo ""
  echo "Options:"
  echo "  -l, --ledger KIND   Specify ledger: hallucinations, ambiguities, dead_ends, boundary_drifts, or all (default: all)"
  echo "  -s, --status STATUS Filter by status (e.g., open, resolved)"
  echo "  -t, --tag TAG       Filter by a specific tag"
  echo "  -d, --days N        Show only entries from the last N days"
  echo "  --latest N          Show only the N most recent entries"
  echo "  -h, --help          Show this help message"
  exit 0
}

# Defaults
LEDGER="all"
STATUS_FILTER=""
TAG_FILTER=""
DAYS_FILTER=""
LATEST_FILTER=""
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LEDGER_DIR="${SCRIPT_DIR}/../ledgers"

# Parse args
while [[ $# -gt 0 ]]; do
  case $1 in
    -l|--ledger) LEDGER="$2"; shift 2 ;;
    -s|--status) STATUS_FILTER="$2"; shift 2 ;;
    -t|--tag) TAG_FILTER="$2"; shift 2 ;;
    -d|--days) DAYS_FILTER="$2"; shift 2 ;;
    --latest) LATEST_FILTER="$2"; shift 2 ;;
    -h|--help) show_help ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

LEDGERS_TO_READ=()
if [ "$LEDGER" = "all" ]; then
  LEDGERS_TO_READ=("hallucinations.jsonl" "ambiguities.jsonl" "dead_ends.jsonl" "boundary_drifts.jsonl")
else
  LEDGERS_TO_READ=("${LEDGER}.jsonl")
fi

echo "=== DEOS Alignment Ledger Summary ==="
if [ -n "$STATUS_FILTER" ]; then echo "Status: $STATUS_FILTER"; fi
if [ -n "$TAG_FILTER" ]; then echo "Tag: $TAG_FILTER"; fi
echo "---------------------------------------"

for L in "${LEDGERS_TO_READ[@]}"; do
  FILE="${LEDGER_DIR}/${L}"
  if [ -s "$FILE" ]; then
    JQ_FILTER="."
    if [ -n "$STATUS_FILTER" ]; then
      JQ_FILTER="${JQ_FILTER} | select(.status == \"$STATUS_FILTER\" or .status == null)"
    fi
    if [ -n "$TAG_FILTER" ]; then
      JQ_FILTER="${JQ_FILTER} | select(.tags != null and (.tags | contains(\"$TAG_FILTER\")))"
    fi
    if [ -n "$DAYS_FILTER" ]; then
      CUTOFF=$(date -d "-${DAYS_FILTER} days" -u +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || date -v-${DAYS_FILTER}d -u +"%Y-%m-%dT%H:%M:%SZ")
      JQ_FILTER="${JQ_FILTER} | select(.timestamp >= \"$CUTOFF\")"
    fi

    echo ">> [ $(echo "$L" | sed 's/\.jsonl//' | tr '[:lower:]' '[:upper:]') ]"

    # Simpler JQ expression without the unsupported default/1 function
    JQ_CMD="jq -c -r '${JQ_FILTER} | \"[\(if .timestamp then .timestamp[0:10] else \"no-date\" end)] \(if .status then (.status | ascii_upcase) else \"WARN\" end): \(if .title then .title elif .rule then .rule else \"Untitled\" end) - \(if .summary then .summary elif .files then .files else \"\" end)\"' \"$FILE\""

    if [ -n "$LATEST_FILTER" ]; then
      eval "$JQ_CMD" | tail -n "$LATEST_FILTER" | awk '{print "  " $0}'
    else
      eval "$JQ_CMD" | awk '{print "  " $0}'
    fi
    echo ""
  fi
done
