#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

WS_URI="${WS_URI:-ws://127.0.0.1:9988}"
WASM_PATH="${WASM_PATH:-$TEMPLATE_DIR/target/release/wbuild/tmctol-runtime/tmctol_runtime.compact.compressed.wasm}"
JSON_OUTPUT="${JSON_OUTPUT:-0}"
BUILD_RUNTIME="${BUILD_RUNTIME:-0}"
SUBMIT_UPGRADE="${SUBMIT_UPGRADE:-0}"
SIGNER_URI="${SIGNER_URI:-//Alice}"
WEB_CLIENT_DIR="$PROJECT_ROOT/web-client"

usage() {
    cat <<'EOF'
Usage: apply-authorized-upgrade-local.sh [OPTIONS]

Verify that a local runtime WASM blob matches the currently authorized runtime-upgrade hash,
and optionally relay the already-authorized code bytes through `System.apply_authorized_upgrade { code }`.

This helper is the operator-facing companion to `check-authorized-upgrade-local.sh`:
  1. Governance authorizes one `code_hash` through `System.authorize_upgrade { code_hash }`
  2. This helper verifies that the local WASM matches that authorized hash
  3. Only with explicit `--submit` does it relay the matching code bytes through `System.apply_authorized_upgrade { code }`

Safety:
  - Default mode is plan-only and does NOT submit anything
  - `--submit` performs the live relay step and may apply the authorized runtime upgrade immediately
  - This local helper is intended for dev/operator-controlled environments; the browser governance surface does not expose this write path

Options:
  --ws URI          WebSocket endpoint to query/submit against (default: ws://127.0.0.1:9988)
  --wasm PATH       Local runtime WASM blob to verify and optionally submit
  --build-runtime   Run ./scripts/03-build-runtime.sh before verification
  --submit          Submit the live `System.apply_authorized_upgrade { code }` relay when the local WASM matches
  --signer-uri URI  Local dev signer URI for `--submit` (default: //Alice)
  --json            Emit machine-readable JSON instead of human-readable output
  -h, --help        Show this help message

Environment:
  WS_URI=ws://127.0.0.1:9988
  WASM_PATH=<path-to-runtime-wasm>
  JSON_OUTPUT=0|1
  BUILD_RUNTIME=0|1
  SUBMIT_UPGRADE=0|1
  SIGNER_URI=//Alice
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
            --submit)
                SUBMIT_UPGRADE=1
                ;;
            --signer-uri)
                [[ $# -ge 2 ]] || {
                    log_error "Missing value for --signer-uri"
                    usage
                    exit 1
                }
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
    phase_banner "Step 2: Relay plan"
    echo "  WS URI:   $WS_URI"
    echo "  WASM:     $WASM_PATH"
    echo "  Build:    $([[ "$BUILD_RUNTIME" == "1" ]] && echo yes || echo no)"
    echo "  Submit:   $([[ "$SUBMIT_UPGRADE" == "1" ]] && echo yes || echo no)"
    echo "  Signer:   $([[ "$SUBMIT_UPGRADE" == "1" ]] && echo "$SIGNER_URI" || echo skipped)"
    echo "  Output:   $([[ "$JSON_OUTPUT" == "1" ]] && echo JSON || echo human)"
}

build_runtime_if_requested() {
    if [[ "$BUILD_RUNTIME" != "1" ]]; then
        return 0
    fi
    phase_banner "Step 3: Build runtime"
    run_script_step "Build runtime" "03-build-runtime.sh"
}

verify_and_maybe_submit() {
    phase_banner "Step 4: Authorized upgrade relay"
    (
        cd "$WEB_CLIENT_DIR"
        WS_URI="$WS_URI" \
        WASM_PATH="$WASM_PATH" \
        JSON_OUTPUT="$JSON_OUTPUT" \
        SUBMIT_UPGRADE="$SUBMIT_UPGRADE" \
        SIGNER_URI="$SIGNER_URI" \
        node --input-type=module <<'EOF'
import { readFile } from "node:fs/promises";
import { blake2AsHex, cryptoWaitReady } from "@polkadot/util-crypto";
import { createWsClient } from "polkadot-api/ws";
import { deos } from "@polkadot-api/descriptors";
import { Keyring } from "@polkadot/keyring";
import { getPolkadotSigner } from "@polkadot-api/signer";

const wsUri = process.env.WS_URI;
const wasmPath = process.env.WASM_PATH;
const jsonOutput = process.env.JSON_OUTPUT === "1";
const submitUpgrade = process.env.SUBMIT_UPGRADE === "1";
const signerUri = process.env.SIGNER_URI;
const wasmBytes = await readFile(wasmPath);
const localCodeHash = blake2AsHex(wasmBytes, 256);
const client = createWsClient(wsUri);
let typedApi;
try {
  typedApi = client.getTypedApi(deos);
  const authorization = await typedApi.view.Governance.authorized_runtime_upgrade();
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
  let submission = null;
  if (submitUpgrade) {
    await cryptoWaitReady();
    const keyring = new Keyring({ type: "sr25519", ss58Format: 42 });
    const pair = keyring.createFromUri(signerUri, { name: signerUri }, "sr25519");
    const signer = getPolkadotSigner(pair.publicKey, "Sr25519", (input) => pair.sign(input));
    const result = await typedApi.tx.System.apply_authorized_upgrade({ code: wasmBytes }).signAndSubmit(signer);
    submission = {
      signerUri,
      txHash: result.txHash,
      ok: result.ok,
      block: result.block,
      events: result.events.length,
    };
  }
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
    submitted: submitUpgrade,
    submission,
    nextStep: submitUpgrade
      ? "Relay submitted"
      : phase === "ready-to-relay-code"
        ? "Rerun with --submit to relay the already-authorized code bytes"
        : phase === "authorized-hash-mismatch"
          ? "Build or point the helper at the exact authorized runtime WASM blob before submitting"
          : "Wait for governance to authorize a runtime-upgrade code hash before submitting",
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
    console.log(`Operator phase:             ${payload.phase}`);
    console.log(`Submit mode:                ${payload.submitted ? "live relay" : "plan-only"}`);
    if (payload.submission) {
      console.log(`Signer URI:                 ${payload.submission.signerUri}`);
      console.log(`Submission tx hash:         ${payload.submission.txHash}`);
      console.log(`Submission ok:              ${payload.submission.ok ? "yes" : "no"}`);
      console.log(`Submission block:           ${payload.submission.block}`);
      console.log(`Submission events:          ${payload.submission.events}`);
    }
    console.log(`Next step:                  ${payload.nextStep}`);
  }
} finally {
  client.destroy();
}
EOF
    )
    log_success "Authorized upgrade relay flow completed"
}

main() {
    parse_args "$@"
    phase_banner "DEOS authorized-upgrade relay"
    check_prerequisites
    print_plan
    build_runtime_if_requested
    verify_and_maybe_submit
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
