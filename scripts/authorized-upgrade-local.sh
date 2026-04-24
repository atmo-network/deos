#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

MODE="${MODE:-check}"
WS_URI="${WS_URI:-ws://127.0.0.1:9988}"
WASM_PATH="${WASM_PATH:-$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm}"
JSON_OUTPUT="${JSON_OUTPUT:-0}"
BUILD_RUNTIME="${BUILD_RUNTIME:-0}"
INCLUDE_CALL_DATA="${INCLUDE_CALL_DATA:-0}"
SUBMIT_UPGRADE="${SUBMIT_UPGRADE:-0}"
SIGNER_URI="${SIGNER_URI:-//Alice}"
WEB_CLIENT_DIR="$PROJECT_ROOT/web-client"

usage() {
    cat <<'EOF'
Usage: authorized-upgrade-local.sh <check|apply> [OPTIONS]

Plan, verify, and optionally relay a governance-authorized runtime upgrade.

Subcommands:
  check              Read governance authorized-upgrade state and verify a local WASM hash
  apply              Verify the local WASM hash and optionally submit System.apply_authorized_upgrade

Options:
  --ws URI           WebSocket endpoint (default: ws://127.0.0.1:9988)
  --wasm PATH        Local runtime WASM blob
  --build-runtime    Run ./scripts/03-build-runtime.sh before verification
  --json             Emit machine-readable JSON
  -h, --help         Show this help message

check options:
  --include-call-data  Emit offline apply_authorized_upgrade call data when hashes match

apply options:
  --submit            Submit the live apply_authorized_upgrade relay when hashes match
  --signer-uri URI    Local dev signer URI for --submit (default: //Alice)

Environment:
  MODE=check|apply
  WS_URI=ws://127.0.0.1:9988
  WASM_PATH=<path-to-runtime-wasm>
  JSON_OUTPUT=0|1
  BUILD_RUNTIME=0|1
  INCLUDE_CALL_DATA=0|1
  SUBMIT_UPGRADE=0|1
  SIGNER_URI=//Alice

Safety:
  Default mode is plan-only. The script submits only with: apply --submit.
EOF
}

parse_args() {
    if [[ $# -gt 0 && "$1" != -* ]]; then
        MODE="$1"
        shift
    fi
    case "$MODE" in
        check|apply) ;;
        *)
            log_error "Unknown subcommand: $MODE"
            usage
            exit 1
            ;;
    esac
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --ws)
                [[ $# -ge 2 ]] || { log_error "Missing value for --ws"; usage; exit 1; }
                WS_URI="$2"
                shift
                ;;
            --wasm)
                [[ $# -ge 2 ]] || { log_error "Missing value for --wasm"; usage; exit 1; }
                WASM_PATH="$2"
                shift
                ;;
            --build-runtime)
                BUILD_RUNTIME=1
                ;;
            --include-call-data)
                INCLUDE_CALL_DATA=1
                ;;
            --submit)
                SUBMIT_UPGRADE=1
                ;;
            --signer-uri)
                [[ $# -ge 2 ]] || { log_error "Missing value for --signer-uri"; usage; exit 1; }
                SIGNER_URI="$2"
                shift
                ;;
            --json)
                JSON_OUTPUT=1
                ;;
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
    if [[ "$MODE" == "check" && "$SUBMIT_UPGRADE" == "1" ]]; then
        log_error "--submit is valid only with the apply subcommand"
        exit 1
    fi
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_directory "$WEB_CLIENT_DIR" "web-client workspace"
    hydrate_local_tool_paths
    require_commands node
    if [[ "$BUILD_RUNTIME" != "1" && ! -f "$WASM_PATH" ]]; then
        log_error "Runtime WASM artifact not found: $WASM_PATH"
        echo "  Hint: run ./scripts/03-build-runtime.sh, rerun with --build-runtime, or pass --wasm <path>"
        exit 1
    fi
    if [[ ! -d "$WEB_CLIENT_DIR/node_modules/polkadot-api" ]]; then
        log_error "web-client dependencies are not installed"
        echo "  Hint: cd web-client && npm install"
        exit 1
    fi
    if [[ ! -d "$WEB_CLIENT_DIR/.papi/descriptors" ]]; then
        log_error "PAPI descriptors not found: $WEB_CLIENT_DIR/.papi/descriptors"
        echo "  Hint: cd web-client && npm run papi:generate"
        exit 1
    fi
    log_success "Prerequisites satisfied"
}

print_plan() {
    phase_banner "Step 2: Authorized-upgrade plan"
    echo "  Mode:    $MODE"
    echo "  WS URI:  $WS_URI"
    echo "  WASM:    $WASM_PATH"
    echo "  Build:   $([[ "$BUILD_RUNTIME" == "1" ]] && echo yes || echo no)"
    echo "  Call:    $([[ "$MODE" == "check" && "$INCLUDE_CALL_DATA" == "1" ]] && echo include || echo skip)"
    echo "  Submit:  $([[ "$MODE" == "apply" && "$SUBMIT_UPGRADE" == "1" ]] && echo yes || echo no)"
    echo "  Signer:  $([[ "$MODE" == "apply" && "$SUBMIT_UPGRADE" == "1" ]] && echo "$SIGNER_URI" || echo skipped)"
    echo "  Output:  $([[ "$JSON_OUTPUT" == "1" ]] && echo JSON || echo human)"
}

build_runtime_if_requested() {
    if [[ "$BUILD_RUNTIME" != "1" ]]; then
        return 0
    fi
    phase_banner "Step 3: Build runtime"
    run_script_step "Build runtime" "03-build-runtime.sh"
}

run_node_flow() {
    phase_banner "Step 4: Authorized upgrade flow"
    (
        cd "$WEB_CLIENT_DIR"
        MODE="$MODE" \
        WS_URI="$WS_URI" \
        WASM_PATH="$WASM_PATH" \
        JSON_OUTPUT="$JSON_OUTPUT" \
        INCLUDE_CALL_DATA="$INCLUDE_CALL_DATA" \
        SUBMIT_UPGRADE="$SUBMIT_UPGRADE" \
        SIGNER_URI="$SIGNER_URI" \
        node --input-type=module <<'EOF'
import { readFile } from "node:fs/promises";
import { u8aToHex } from "@polkadot/util";
import { blake2AsHex, cryptoWaitReady } from "@polkadot/util-crypto";
import { createWsClient } from "polkadot-api/ws";
import { deos } from "@polkadot-api/descriptors";
import { Keyring } from "@polkadot/keyring";
import { getPolkadotSigner } from "@polkadot-api/signer";

const mode = process.env.MODE;
const wsUri = process.env.WS_URI;
const wasmPath = process.env.WASM_PATH;
const jsonOutput = process.env.JSON_OUTPUT === "1";
const includeCallData = process.env.INCLUDE_CALL_DATA === "1";
const submitUpgrade = mode === "apply" && process.env.SUBMIT_UPGRADE === "1";
const signerUri = process.env.SIGNER_URI;
const wasmBytes = await readFile(wasmPath);
const localCodeHash = blake2AsHex(wasmBytes, 256);
const client = createWsClient(wsUri);
try {
  const api = client.getTypedApi(deos);
  const authorization = await api.view.Governance.authorized_runtime_upgrade();
  const matchesAuthorizedHash = authorization
    ? authorization.code_hash.toLowerCase() === localCodeHash.toLowerCase()
    : false;
  const phase = authorization
    ? matchesAuthorizedHash
      ? "ready-to-relay-code"
      : "authorized-hash-mismatch"
    : "awaiting-governance-authorization";
  if (submitUpgrade && phase !== "ready-to-relay-code") {
    throw new Error(`Refusing to submit apply_authorized_upgrade while phase=${phase}`);
  }
  const applyCallData = mode === "check" && includeCallData && matchesAuthorizedHash
    ? await api.tx.System.apply_authorized_upgrade({ code: wasmBytes }).getEncodedData()
    : null;
  let submission = null;
  if (submitUpgrade) {
    await cryptoWaitReady();
    const keyring = new Keyring({ type: "sr25519", ss58Format: 42 });
    const pair = keyring.createFromUri(signerUri, { name: signerUri }, "sr25519");
    const signer = getPolkadotSigner(pair.publicKey, "Sr25519", (input) => pair.sign(input));
    const result = await api.tx.System.apply_authorized_upgrade({ code: wasmBytes }).signAndSubmit(signer);
    submission = {
      signerUri,
      txHash: result.txHash,
      ok: result.ok,
      block: result.block,
      events: result.events.length,
    };
  }
  const recommendedAction = authorization
    ? matchesAuthorizedHash
      ? mode === "apply"
        ? submitUpgrade
          ? "Relay submitted"
          : "Rerun with apply --submit to relay the already-authorized code bytes"
        : includeCallData
          ? "Submit the emitted call data externally if you intend to relay the already-authorized code bytes"
          : "Rerun with check --include-call-data if you want offline apply_authorized_upgrade call data"
      : "Build or point the helper at the exact authorized runtime WASM blob before attempting the relay step"
    : "Wait for governance to authorize a runtime-upgrade code hash before preparing a relay";
  const payload = {
    mode,
    wsUri,
    wasmPath,
    wasmByteLength: wasmBytes.length,
    localCodeHash,
    authorizedUpgrade: authorization
      ? { codeHash: authorization.code_hash, checkVersion: authorization.check_version }
      : null,
    matchesAuthorizedHash,
    phase,
    submitted: submitUpgrade,
    submission,
    applyAuthorizedUpgradeCallData: applyCallData ? u8aToHex(applyCallData) : null,
    applyAuthorizedUpgradeCallDataByteLength: applyCallData?.length ?? null,
    recommendedAction,
    operatorPath: {
      authorizationAuthority: "Governance authorizes the pending code hash through System.authorize_upgrade",
      applicationAuthority: "Any origin may relay matching code bytes through System.apply_authorized_upgrade after authorization",
      browserSubmissionSurface: "Not exposed in the web-client",
      helperSubmissionSurface: mode === "apply" ? "Plan-only unless --submit is provided" : "Plan-only verifier and optional offline call-data emitter",
    },
  };
  if (jsonOutput) {
    console.log(JSON.stringify(payload, null, 2));
  } else {
    console.log(`Authorized upgrade present: ${payload.authorizedUpgrade ? "yes" : "no"}`);
    console.log(`Local WASM bytes:           ${payload.wasmByteLength}`);
    console.log(`Local code hash:            ${payload.localCodeHash}`);
    if (payload.authorizedUpgrade) {
      console.log(`Authorized code hash:       ${payload.authorizedUpgrade.codeHash}`);
      console.log(`Version check:              ${payload.authorizedUpgrade.checkVersion ? "required" : "disabled"}`);
      console.log(`Hash match:                 ${payload.matchesAuthorizedHash ? "yes" : "no"}`);
    }
    if (payload.applyAuthorizedUpgradeCallData) {
      console.log(`Call data bytes:            ${payload.applyAuthorizedUpgradeCallDataByteLength}`);
      console.log(`Call data hex:              ${payload.applyAuthorizedUpgradeCallData}`);
    }
    console.log(`Operator phase:             ${payload.phase}`);
    console.log(`Submit mode:                ${payload.submitted ? "live relay" : "plan-only"}`);
    if (payload.submission) {
      console.log(`Signer URI:                 ${payload.submission.signerUri}`);
      console.log(`Submission tx hash:         ${payload.submission.txHash}`);
      console.log(`Submission ok:              ${payload.submission.ok ? "yes" : "no"}`);
      console.log(`Submission block:           ${payload.submission.block}`);
      console.log(`Submission events:          ${payload.submission.events}`);
    }
    console.log(`Governance step:            ${payload.operatorPath.authorizationAuthority}`);
    console.log(`Relay step:                 ${payload.operatorPath.applicationAuthority}`);
    console.log(`Browser path:               ${payload.operatorPath.browserSubmissionSurface}`);
    console.log(`Helper path:                ${payload.operatorPath.helperSubmissionSurface}`);
    console.log(`Recommended action:         ${payload.recommendedAction}`);
  }
} finally {
  client.destroy();
}
EOF
    )
    log_success "Authorized upgrade flow completed"
}

main() {
    parse_args "$@"
    phase_banner "DEOS authorized-upgrade local tool"
    check_prerequisites
    print_plan
    build_runtime_if_requested
    run_node_flow
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
