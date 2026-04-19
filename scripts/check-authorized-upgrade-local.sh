#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

WS_URI="${WS_URI:-ws://127.0.0.1:9988}"
WASM_PATH="${WASM_PATH:-$TEMPLATE_DIR/target/release/wbuild/tmctol-runtime/tmctol_runtime.compact.compressed.wasm}"
JSON_OUTPUT="${JSON_OUTPUT:-0}"
BUILD_RUNTIME="${BUILD_RUNTIME:-0}"
INCLUDE_CALL_DATA="${INCLUDE_CALL_DATA:-0}"
WEB_CLIENT_DIR="$PROJECT_ROOT/web-client"

usage() {
    cat <<'EOF'
Usage: check-authorized-upgrade-local.sh [OPTIONS]

Read the chain's current canonical governance/runtime authorized-upgrade state, hash a local runtime WASM blob,
and report whether the local code matches the pending authorized runtime-upgrade hash.

This helper makes the current launch-line role split explicit:
  1. Governance authorizes one `code_hash` through `System.authorize_upgrade { code_hash }`
  2. Any origin may later relay matching code bytes through `System.apply_authorized_upgrade { code }`
  3. This helper can verify and emit offline call data, but it does NOT submit the live call

Options:
  --ws URI             WebSocket endpoint to query (default: ws://127.0.0.1:9988)
  --wasm PATH          Local runtime WASM blob to verify
  --build-runtime      Run ./scripts/03-build-runtime.sh before verification
  --include-call-data  When the local WASM matches the authorized hash, also emit offline `apply_authorized_upgrade` call data
  --json               Emit machine-readable JSON instead of human-readable output
  -h, --help           Show this help message

Environment:
  WS_URI=ws://127.0.0.1:9988
  WASM_PATH=<path-to-runtime-wasm>
  JSON_OUTPUT=0|1
  BUILD_RUNTIME=0|1
  INCLUDE_CALL_DATA=0|1
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --ws)
                [[ $# -ge 2 ]] || {
                    log_error "Missing value for --ws"
                    usage
                    exit 1
                }
                WS_URI="$2"
                shift
                ;;
            --wasm)
                [[ $# -ge 2 ]] || {
                    log_error "Missing value for --wasm"
                    usage
                    exit 1
                }
                WASM_PATH="$2"
                shift
                ;;
            --build-runtime)
                BUILD_RUNTIME=1
                ;;
            --include-call-data)
                INCLUDE_CALL_DATA=1
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
    phase_banner "Step 2: Verification plan"
    echo "  WS URI: $WS_URI"
    echo "  WASM:   $WASM_PATH"
    echo "  Build:  $([[ "$BUILD_RUNTIME" == "1" ]] && echo yes || echo no)"
    echo "  Call:   $([[ "$INCLUDE_CALL_DATA" == "1" ]] && echo include || echo skip)"
    echo "  Output: $([[ "$JSON_OUTPUT" == "1" ]] && echo JSON || echo human)"
}

build_runtime_if_requested() {
    if [[ "$BUILD_RUNTIME" != "1" ]]; then
        return 0
    fi
    phase_banner "Step 2: Build runtime"
    run_script_step "Build runtime" "03-build-runtime.sh"
}

query_authorized_upgrade_status() {
    phase_banner "Step 3: Authorized upgrade status"
    (
        cd "$WEB_CLIENT_DIR"
        WS_URI="$WS_URI" WASM_PATH="$WASM_PATH" JSON_OUTPUT="$JSON_OUTPUT" INCLUDE_CALL_DATA="$INCLUDE_CALL_DATA" node --input-type=module <<'EOF'
import { readFile } from "node:fs/promises";
import { u8aToHex } from "@polkadot/util";
import { blake2AsHex } from "@polkadot/util-crypto";
import { createWsClient } from "polkadot-api/ws";
import { tmctol } from "@polkadot-api/descriptors";

const wsUri = process.env.WS_URI;
const wasmPath = process.env.WASM_PATH;
const jsonOutput = process.env.JSON_OUTPUT === "1";
const includeCallData = process.env.INCLUDE_CALL_DATA === "1";
const wasmBytes = await readFile(wasmPath);
const localCodeHash = blake2AsHex(wasmBytes, 256);
const client = createWsClient(wsUri);
let typedApi;
try {
  typedApi = client.getTypedApi(tmctol);
  const authorization = await typedApi.view.Governance.authorized_runtime_upgrade();
  const matchesAuthorizedHash = authorization
    ? authorization.code_hash.toLowerCase() === localCodeHash.toLowerCase()
    : false;
  const applyCallData = includeCallData && matchesAuthorizedHash
    ? await typedApi.tx.System.apply_authorized_upgrade({ code: wasmBytes }).getEncodedData()
    : null;
  const phase = authorization
    ? matchesAuthorizedHash
      ? "ready-to-relay-code"
      : "authorized-hash-mismatch"
    : "awaiting-governance-authorization";
  const recommendedAction = authorization
    ? matchesAuthorizedHash
      ? includeCallData
        ? "Submit the emitted call data through an external system-origin path if you intend to relay the already-authorized code bytes"
        : "Rerun with --include-call-data if you want offline apply_authorized_upgrade call data for the matching code bytes"
      : "Build or point the helper at the exact authorized runtime WASM blob before attempting the relay step"
    : "Wait for governance to authorize a runtime-upgrade code hash before preparing an apply_authorized_upgrade relay";
  const payload = {
    wsUri,
    wasmPath,
    wasmByteLength: wasmBytes.length,
    localCodeHash,
    authorizedUpgrade: authorization
      ? {
          codeHash: authorization.code_hash,
          checkVersion: authorization.check_version,
        }
      : null,
    matchesAuthorizedHash,
    phase,
    recommendedAction,
    applyAuthorizedUpgradeCallData: applyCallData ? u8aToHex(applyCallData) : null,
    applyAuthorizedUpgradeCallDataByteLength: applyCallData?.length ?? null,
    operatorPath: {
      authorizationAuthority: "Governance authorizes the pending code hash through System.authorize_upgrade",
      applicationAuthority: "Any origin may relay matching code bytes through System.apply_authorized_upgrade after authorization",
      browserSubmissionSurface: "Not exposed in the web-client",
      helperSubmissionSurface: "Plan-only verifier and optional offline call-data emitter; never submits the live call",
    },
    nextStep: authorization
      ? matchesAuthorizedHash
        ? includeCallData
          ? "Matching code bytes are ready for external System.apply_authorized_upgrade with the emitted call data; this helper intentionally does not submit that call"
          : "Matching code bytes are ready for external System.apply_authorized_upgrade; rerun with --include-call-data if you want offline call data without submission"
        : "The local WASM does not match the currently authorized code hash; do not apply this blob"
      : "No authorized runtime upgrade is currently pending on-chain",
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
    console.log(`Governance step:            ${payload.operatorPath.authorizationAuthority}`);
    console.log(`Relay step:                 ${payload.operatorPath.applicationAuthority}`);
    console.log(`Browser path:               ${payload.operatorPath.browserSubmissionSurface}`);
    console.log(`Helper path:                ${payload.operatorPath.helperSubmissionSurface}`);
    console.log(`Recommended action:         ${payload.recommendedAction}`);
    console.log(`Next step:                  ${payload.nextStep}`);
  }
} finally {
  client.destroy();
}
EOF
    )
    log_success "Authorized upgrade status checked"
}

main() {
    parse_args "$@"
    phase_banner "DEOS authorized-upgrade verifier"
    check_prerequisites
    print_plan
    build_runtime_if_requested
    query_authorized_upgrade_status
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
