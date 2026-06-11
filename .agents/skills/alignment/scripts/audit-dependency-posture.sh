#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-dependency-posture.sh [OPTIONS]

Reports dependency posture for package-managed workspaces without applying fixes.
Fails on in-range npm drift where current != wanted, on moderate-or-higher npm
audit findings, and on unexpected template cargo-update failures. Low-severity
advisory paths and the current known template cargo-update blocker stay
report-only because they require explicit non-regressive upstream evaluation.

Options:
  -h, --help  Show this help message
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -h|--help)
                usage
                exit 0
                ;;
            *)
                log_error "Unknown argument: $1"
                usage
                exit 1
                ;;
        esac
        shift
    done
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_commands node npm grep mktemp
    log_success "Prerequisites checked"
}

read_npm_json_report() {
    local label="$1"
    local workdir="$2"
    local allowed_exit_codes="$3"
    shift 3

    local stdout_file
    local stderr_file
    local exit_code
    stdout_file="$(mktemp)"
    stderr_file="$(mktemp)"
    set +e
    (cd "$workdir" && "$@" --json >"$stdout_file" 2>"$stderr_file")
    exit_code=$?
    set -e

    if [[ ",$allowed_exit_codes," != *",$exit_code,"* ]]; then
        log_error "$label npm command failed unexpectedly: $* --json"
        echo "Exit code: $exit_code"
        if [[ -s "$stderr_file" ]]; then
            echo "stderr:"
            cat "$stderr_file"
        fi
        if [[ -s "$stdout_file" ]]; then
            echo "stdout:"
            cat "$stdout_file"
        fi
        rm -f "$stdout_file" "$stderr_file"
        exit 1
    fi

    if [[ ! -s "$stdout_file" ]]; then
        log_error "$label npm command produced no JSON output: $* --json"
        if [[ -s "$stderr_file" ]]; then
            echo "stderr:"
            cat "$stderr_file"
        fi
        rm -f "$stdout_file" "$stderr_file"
        exit 1
    fi

    cat "$stdout_file"
    rm -f "$stdout_file" "$stderr_file"
}

check_npm_outdated() {
    local label="$1"
    local workdir="$2"

    if [[ ! -f "$workdir/package.json" ]]; then
        log_info "Skipping $label npm outdated check: package.json not found"
        return 0
    fi

    phase_banner "Step 2: $label npm outdated"
    local report
    report="$(read_npm_json_report "$label" "$workdir" "0,1" npm outdated)"

    REPORT_JSON="$report" WORKSPACE_LABEL="$label" node <<'EOF'
let report;
try {
  report = JSON.parse(process.env.REPORT_JSON ?? '');
} catch (error) {
  console.error(`[ERROR] Unable to parse npm outdated JSON: ${error.message}`);
  process.exit(1);
}
if (!report || Array.isArray(report) || typeof report !== 'object') {
  console.error('[ERROR] npm outdated returned an unexpected JSON shape');
  process.exit(1);
}
const label = process.env.WORKSPACE_LABEL;
const wantedDrift = [];
const latestDrift = [];
const knownCompatibilityWatch = [];
for (const [name, info] of Object.entries(report)) {
  if (info.current !== info.wanted) {
    wantedDrift.push(`${name}: current=${info.current} wanted=${info.wanted}`);
  } else if (info.current !== info.latest) {
    if (label === 'web-client' && name === 'prettier-plugin-svelte') {
      knownCompatibilityWatch.push(
        `${name}: current=${info.current} latest=${info.latest} (watch only: @trivago/prettier-plugin-sort-imports 6.0.2 optional peer is prettier-plugin-svelte 3.x)`,
      );
    } else {
      latestDrift.push(`${name}: current=${info.current} latest=${info.latest}`);
    }
  }
}
if (wantedDrift.length > 0) {
  console.error(`[ERROR] ${label} has in-range dependency drift:`);
  for (const line of wantedDrift) console.error(`  - ${line}`);
  process.exit(1);
}
if (latestDrift.length > 0) {
  console.log(`[INFO] ${label} latest-version drift requiring explicit evaluation:`);
  for (const line of latestDrift) console.log(`  - ${line}`);
}
if (knownCompatibilityWatch.length > 0) {
  console.log(`[INFO] ${label} known formatter compatibility watch:`);
  for (const line of knownCompatibilityWatch) console.log(`  - ${line}`);
}
if (latestDrift.length === 0 && knownCompatibilityWatch.length === 0) {
  console.log(`[SUCCESS] ${label} has no npm outdated drift`);
}
EOF
}

check_template_cargo_update() {
    if [[ ! -f "$PROJECT_ROOT/template/Cargo.toml" ]]; then
        log_info "Skipping template cargo update dry-run: template/Cargo.toml not found"
        return 0
    fi
    if ! command -v cargo >/dev/null 2>&1; then
        log_warning "Skipping template cargo update dry-run: cargo not found"
        return 0
    fi

    phase_banner "Step 3: template cargo update dry-run"
    local output
    if output="$(cargo update --manifest-path "$PROJECT_ROOT/template/Cargo.toml" --dry-run 2>&1)"; then
        log_success "template cargo update dry-run completed"
        return 0
    fi
    if grep -q 'core2 = "\^0\.4\.0"' <<<"$output" && grep -q 'version 0\.4\.0 is yanked' <<<"$output"; then
        log_info "template cargo update remains blocked by known yanked core2 v0.4.0 through the current SDK/litep2p dependency chain"
        return 0
    fi
    log_error "Unexpected template cargo update dry-run failure"
    echo "$output"
    exit 1
}

check_web_client_audit() {
    local workdir="$PROJECT_ROOT/web-client"
    if [[ ! -f "$workdir/package.json" ]]; then
        log_info "Skipping web-client npm audit: package.json not found"
        return 0
    fi

    phase_banner "Step 4: web-client npm audit"
    local report
    report="$(read_npm_json_report "web-client" "$workdir" "0,1" npm audit --audit-level=moderate)"

    REPORT_JSON="$report" node <<'EOF'
let report;
try {
  report = JSON.parse(process.env.REPORT_JSON ?? '');
} catch (error) {
  console.error(`[ERROR] Unable to parse npm audit JSON: ${error.message}`);
  process.exit(1);
}
if (report.error) {
  const summary = report.error.summary ?? report.error.code ?? 'unknown npm audit error';
  console.error(`[ERROR] npm audit returned an error report: ${summary}`);
  process.exit(1);
}
if (!report.metadata || !report.metadata.vulnerabilities) {
  console.error('[ERROR] npm audit returned an unexpected JSON shape');
  process.exit(1);
}
const counts = report.metadata.vulnerabilities;
const moderatePlus = ['moderate', 'high', 'critical']
  .reduce((total, key) => total + Number(counts[key] ?? 0), 0);
const low = Number(counts.low ?? 0);
if (moderatePlus > 0) {
  console.error(`[ERROR] web-client has ${moderatePlus} moderate-or-higher npm audit findings`);
  process.exit(1);
}
if (low > 0) {
  const lowNames = Object.entries(report.vulnerabilities ?? {})
    .filter(([, vulnerability]) => vulnerability?.severity === 'low')
    .map(([name]) => name)
    .sort();
  const suffix = lowNames.length > 0 ? `: ${lowNames.join(', ')}` : '';
  console.log(`[INFO] web-client has ${low} low-severity npm audit findings${suffix}; keep the documented non-regressive follow-up open`);
} else {
  console.log('[SUCCESS] web-client has no npm audit findings');
}
EOF
}

main() {
    parse_args "$@"
    check_prerequisites
    check_npm_outdated "root" "$PROJECT_ROOT"
    check_npm_outdated "web-client" "$PROJECT_ROOT/web-client"
    check_template_cargo_update
    check_web_client_audit
    log_success "Dependency posture audit completed"
}

main "$@"
