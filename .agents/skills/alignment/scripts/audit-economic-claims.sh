#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

CLAIMS_FILE="$SKILL_DIR/economic-claims.json"

usage() {
    cat <<'EOF'
Usage: audit-economic-claims.sh [OPTIONS]

Validates the machine-readable economic-claim inventory used by semantic
doc-vs-runtime drift checks. The audit ensures every claim has anchors and that
referenced files/symbols/tests still exist.

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
    if [[ ! -f "$CLAIMS_FILE" ]]; then
        log_error "Economic claims inventory not found: $CLAIMS_FILE"
        exit 1
    fi
    require_commands node
    log_success "Prerequisites checked"
}

run_audit() {
    phase_banner "Step 2: Economic claim inventory"
    CLAIMS_FILE="$CLAIMS_FILE" PROJECT_ROOT="$PROJECT_ROOT" node <<'NODE'
const fs = require('fs');
const path = require('path');

const claimsFile = process.env.CLAIMS_FILE;
const projectRoot = process.env.PROJECT_ROOT;
const data = JSON.parse(fs.readFileSync(claimsFile, 'utf8'));
const errors = [];
const allowedProofKinds = new Set(['falsification', 'characterization', 'invariant', 'mitigation']);
const allowedTautologyRisks = new Set(['low', 'medium', 'high']);

function check(condition, message) {
  if (!condition) errors.push(message);
}

function symbolFromAnchor(anchor) {
  const parts = anchor.split('::');
  return parts[parts.length - 1];
}

function validateAnchor(claimId, field, anchor) {
  const [relativePath] = anchor.split('::');
  check(relativePath && relativePath !== anchor, `${claimId}: ${field} anchor must use path::symbol syntax: ${anchor}`);
  if (!relativePath) return;
  const absolutePath = path.join(projectRoot, relativePath);
  if (!fs.existsSync(absolutePath)) {
    errors.push(`${claimId}: ${field} file does not exist: ${relativePath}`);
    return;
  }
  const symbol = symbolFromAnchor(anchor);
  const content = fs.readFileSync(absolutePath, 'utf8');
  check(content.includes(symbol), `${claimId}: ${field} symbol not found in ${relativePath}: ${symbol}`);
}

check(data && data.schema_version === 1, 'schema_version must be 1');
check(Array.isArray(data.claims), 'claims must be an array');
const ids = new Set();
for (const claim of data.claims || []) {
  check(claim.id && typeof claim.id === 'string', 'claim id is required');
  if (!claim.id) continue;
  check(!ids.has(claim.id), `duplicate claim id: ${claim.id}`);
  ids.add(claim.id);
  check(claim.doc && typeof claim.doc === 'string', `${claim.id}: doc is required`);
  if (claim.doc) check(fs.existsSync(path.join(projectRoot, claim.doc)), `${claim.id}: doc file does not exist: ${claim.doc}`);
  check(claim.claim && typeof claim.claim === 'string', `${claim.id}: claim text is required`);
  check(claim.status && typeof claim.status === 'string', `${claim.id}: status is required`);
  check(allowedProofKinds.has(claim.proof_kind), `${claim.id}: proof_kind must be one of ${Array.from(allowedProofKinds).join(', ')}`);
  check(allowedTautologyRisks.has(claim.tautology_risk), `${claim.id}: tautology_risk must be one of ${Array.from(allowedTautologyRisks).join(', ')}`);
  check(claim.falsification_note && typeof claim.falsification_note === 'string' && claim.falsification_note.length >= 24, `${claim.id}: falsification_note must explain what would break the test`);
  check(claim.tautology_risk !== 'high', `${claim.id}: high tautology risk cannot pass the release gate`);
  check(Array.isArray(claim.code_anchors) && claim.code_anchors.length > 0, `${claim.id}: code_anchors must be non-empty`);
  check(Array.isArray(claim.falsification_tests) && claim.falsification_tests.length > 0, `${claim.id}: falsification_tests must be non-empty`);
  for (const anchor of claim.code_anchors || []) validateAnchor(claim.id, 'code', anchor);
  for (const anchor of claim.falsification_tests || []) validateAnchor(claim.id, 'test', anchor);
}

if (errors.length > 0) {
  for (const error of errors) console.error(error);
  process.exit(1);
}
console.log(`Validated ${data.claims.length} economic claims`);
NODE
    log_success "Economic claim inventory passed"
}

main() {
    parse_args "$@"
    check_prerequisites
    run_audit
}

main "$@"
