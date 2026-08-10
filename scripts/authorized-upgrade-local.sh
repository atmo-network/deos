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
UPGRADE_STATE_PATH="${UPGRADE_STATE_PATH:-${TMPDIR:-/tmp}/deos-upgrade-state.json}"
UPGRADE_SOURCE_SPEC_VERSION="${UPGRADE_SOURCE_SPEC_VERSION:-1}"
UPGRADE_TARGET_SPEC_VERSION="${UPGRADE_TARGET_SPEC_VERSION:-2}"
UPGRADE_FOREIGN_ID="${UPGRADE_FOREIGN_ID:-4026531841}"
GOVERNANCE_DOMAIN_ID="${GOVERNANCE_DOMAIN_ID:-0}"
VETO_ASSET_ID="${VETO_ASSET_ID:-268435457}"
PROPOSER_ADDRESS="${PROPOSER_ADDRESS:-}"
PROPOSAL_ITEM_ID="${PROPOSAL_ITEM_ID:-}"
STRATEGIC_STAKE_AMOUNT="${STRATEGIC_STAKE_AMOUNT:-5000000000000}"
WEB_CLIENT_DIR="$PROJECT_ROOT/web-client"

usage() {
    cat <<'EOF'
Usage: authorized-upgrade-local.sh <check|prepare-authorization|apply|snapshot|verify> [OPTIONS]

Plan, verify, and optionally relay a governance-authorized runtime upgrade, or
capture/verify finalized non-empty state for a deployed downstream runtime.

Subcommands:
  check              Read strategic-ingress/authorized-upgrade state and verify local WASM
  prepare-authorization
                     Emit plan-only signed call data for candidate proposal creation
  apply              Verify and optionally relay System.apply_authorized_upgrade
  snapshot           Capture finalized baseline Router/Oracle/Actors state
  verify             Verify candidate code/version and exact baseline-state preservation

Options:
  --ws URI           WebSocket endpoint (default: ws://127.0.0.1:9988)
  --wasm PATH        Local runtime WASM blob
  --build-runtime    Run ./scripts/03-build-runtime.sh before verification
  --json             Emit machine-readable JSON for check/prepare-authorization/apply
  --state PATH       Baseline state evidence path
  --proposer-address SS58
                     Strategic proposal signer for prepare-authorization
  --item-id ID       Unused governance item id for prepare-authorization
  --stake-amount AMOUNT
                     NTVE stake amount planned when the signer lacks receipt stake
  -h, --help         Show this help message

check options:
  --include-call-data  Emit offline apply_authorized_upgrade call data when hashes match

apply options:
  --submit            Submit the live apply_authorized_upgrade relay when hashes match
  --signer-uri URI    Local dev signer URI for --submit (default: //Alice)

Environment:
  MODE=check|prepare-authorization|apply|snapshot|verify
  WS_URI=ws://127.0.0.1:9988
  WASM_PATH=<path-to-runtime-wasm>
  JSON_OUTPUT=0|1
  BUILD_RUNTIME=0|1
  INCLUDE_CALL_DATA=0|1
  SUBMIT_UPGRADE=0|1
  SIGNER_URI=//Alice
  UPGRADE_STATE_PATH=/tmp/deos-upgrade-state.json
  UPGRADE_SOURCE_SPEC_VERSION=1
  UPGRADE_TARGET_SPEC_VERSION=2
  UPGRADE_FOREIGN_ID=4026531841
  GOVERNANCE_DOMAIN_ID=0
  VETO_ASSET_ID=268435457
  PROPOSER_ADDRESS=<SS58>
  PROPOSAL_ITEM_ID=<u32>
  STRATEGIC_STAKE_AMOUNT=5000000000000

Safety:
  check, prepare-authorization, snapshot, and verify never submit. The script
  submits only with: apply --submit.
EOF
}

parse_args() {
    if [[ $# -gt 0 && "$1" != -* ]]; then
        MODE="$1"
        shift
    fi
    case "$MODE" in
        check|prepare-authorization|apply|snapshot|verify) ;;
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
            --state)
                [[ $# -ge 2 ]] || { log_error "Missing value for --state"; usage; exit 1; }
                UPGRADE_STATE_PATH="$2"
                shift
                ;;
            --proposer-address)
                [[ $# -ge 2 ]] || { log_error "Missing value for --proposer-address"; usage; exit 1; }
                PROPOSER_ADDRESS="$2"
                shift
                ;;
            --item-id)
                [[ $# -ge 2 ]] || { log_error "Missing value for --item-id"; usage; exit 1; }
                PROPOSAL_ITEM_ID="$2"
                shift
                ;;
            --stake-amount)
                [[ $# -ge 2 ]] || { log_error "Missing value for --stake-amount"; usage; exit 1; }
                STRATEGIC_STAKE_AMOUNT="$2"
                shift
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
    if [[ "$MODE" != "apply" && "$SUBMIT_UPGRADE" == "1" ]]; then
        log_error "--submit is valid only with the apply subcommand"
        exit 1
    fi
    if [[ "$MODE" != "check" && "$INCLUDE_CALL_DATA" == "1" ]]; then
        log_error "--include-call-data is valid only with the check subcommand"
        exit 1
    fi
    if [[ "$MODE" == "prepare-authorization" ]]; then
        [[ -n "$PROPOSER_ADDRESS" ]] || { log_error "--proposer-address is required with prepare-authorization"; exit 1; }
        [[ -n "$PROPOSAL_ITEM_ID" ]] || { log_error "--item-id is required with prepare-authorization"; exit 1; }
    elif [[ -n "$PROPOSER_ADDRESS" || -n "$PROPOSAL_ITEM_ID" ]]; then
        log_error "--proposer-address and --item-id are valid only with prepare-authorization"
        exit 1
    fi
    if [[ "$MODE" == "snapshot" && "$BUILD_RUNTIME" == "1" ]]; then
        log_error "--build-runtime is not used by the snapshot subcommand"
        exit 1
    fi
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_directory "$WEB_CLIENT_DIR" "web-client workspace"
    activate_pinned_node
    hydrate_local_tool_paths
    require_commands node dirname basename
    if [[ "$MODE" != "snapshot" && "$BUILD_RUNTIME" != "1" && ! -f "$WASM_PATH" ]]; then
        log_error "Runtime WASM artifact not found: $WASM_PATH"
        echo "  Hint: run ./scripts/03-build-runtime.sh, rerun with --build-runtime, or pass --wasm <path>"
        exit 1
    fi
    if [[ "$MODE" != "snapshot" && -f "$WASM_PATH" ]]; then
        WASM_PATH="$(cd "$(dirname "$WASM_PATH")" && pwd)/$(basename "$WASM_PATH")"
    fi
    if [[ "$MODE" == "snapshot" || "$MODE" == "verify" ]]; then
        local state_directory
        state_directory="$(dirname "$UPGRADE_STATE_PATH")"
        [[ -d "$state_directory" ]] || { log_error "Upgrade-state directory not found: $state_directory"; exit 1; }
        UPGRADE_STATE_PATH="$(cd "$state_directory" && pwd)/$(basename "$UPGRADE_STATE_PATH")"
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
    echo "  State:   $UPGRADE_STATE_PATH"
    echo "  Domain:  $GOVERNANCE_DOMAIN_ID"
    echo "  VETO:    $VETO_ASSET_ID"
    echo "  Proposer:$([[ "$MODE" == "prepare-authorization" ]] && echo "  $PROPOSER_ADDRESS" || echo "  skipped")"
    echo "  Item:    $([[ "$MODE" == "prepare-authorization" ]] && echo "$PROPOSAL_ITEM_ID" || echo skipped)"
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
    if [[ "$MODE" == "snapshot" || "$MODE" == "verify" ]]; then
        (
            cd "$WEB_CLIENT_DIR"
            MODE="$MODE" \
            WS_URI="$WS_URI" \
            WASM_PATH="$WASM_PATH" \
            UPGRADE_STATE_PATH="$UPGRADE_STATE_PATH" \
            UPGRADE_SOURCE_SPEC_VERSION="$UPGRADE_SOURCE_SPEC_VERSION" \
            UPGRADE_TARGET_SPEC_VERSION="$UPGRADE_TARGET_SPEC_VERSION" \
            UPGRADE_FOREIGN_ID="$UPGRADE_FOREIGN_ID" \
                node scripts/upgrade-state-evidence.mjs
        )
        return 0
    fi
    (
        cd "$WEB_CLIENT_DIR"
        MODE="$MODE" \
        WS_URI="$WS_URI" \
        WASM_PATH="$WASM_PATH" \
        JSON_OUTPUT="$JSON_OUTPUT" \
        INCLUDE_CALL_DATA="$INCLUDE_CALL_DATA" \
        SUBMIT_UPGRADE="$SUBMIT_UPGRADE" \
        SIGNER_URI="$SIGNER_URI" \
        GOVERNANCE_DOMAIN_ID="$GOVERNANCE_DOMAIN_ID" \
        VETO_ASSET_ID="$VETO_ASSET_ID" \
        PROPOSER_ADDRESS="$PROPOSER_ADDRESS" \
        PROPOSAL_ITEM_ID="$PROPOSAL_ITEM_ID" \
        STRATEGIC_STAKE_AMOUNT="$STRATEGIC_STAKE_AMOUNT" \
        node --input-type=module <<'EOF'
import { readFile } from "node:fs/promises";
import { hexToU8a, u8aToHex } from "@polkadot/util";
import { blake2AsHex, cryptoWaitReady } from "@polkadot/util-crypto";
import { createWsClient } from "polkadot-api/ws";
import { deos } from "@polkadot-api/descriptors";
import { Keyring } from "@polkadot/keyring";
import { getPolkadotSigner } from "@polkadot-api/signer";
import { Enum as PapiEnum } from "polkadot-api";

function unsignedLiteral(literal, label) {
  if (!/^(0|[1-9][0-9]*)$/.test(literal)) {
    throw new Error(`${label} must be a complete unsigned integer literal`);
  }
  return literal;
}

function u32(literal, label) {
  const value = Number(unsignedLiteral(literal, label));
  if (!Number.isSafeInteger(value) || value > 0xffffffff) {
    throw new Error(`${label} must fit in u32`);
  }
  return value;
}

function bigUint(literal, label) {
  return BigInt(unsignedLiteral(literal, label));
}

function stringify(value) {
  return JSON.stringify(
    value,
    (_, inner) =>
      typeof inner === "bigint"
        ? inner.toString()
        : inner instanceof Uint8Array
          ? u8aToHex(inner)
          : inner,
    2,
  );
}

async function encoded(label, authority, tx, params) {
  const data = await tx(params).getEncodedData();
  return {
    label,
    authority,
    params,
    callData: u8aToHex(data),
    callDataByteLength: data.length,
  };
}

const mode = process.env.MODE;
const wsUri = process.env.WS_URI;
const wasmPath = process.env.WASM_PATH;
const jsonOutput = process.env.JSON_OUTPUT === "1";
const includeCallData = process.env.INCLUDE_CALL_DATA === "1";
const submitUpgrade = mode === "apply" && process.env.SUBMIT_UPGRADE === "1";
const signerUri = process.env.SIGNER_URI;
const governanceDomainId = u32(process.env.GOVERNANCE_DOMAIN_ID, "governance domain id");
const vetoAssetId = u32(process.env.VETO_ASSET_ID, "VETO asset id");
const proposerAddress = process.env.PROPOSER_ADDRESS;
const proposalItemId = mode === "prepare-authorization"
  ? u32(process.env.PROPOSAL_ITEM_ID, "proposal item id")
  : null;
const strategicStakeAmount = mode === "prepare-authorization"
  ? bigUint(process.env.STRATEGIC_STAKE_AMOUNT, "strategic stake amount")
  : null;
const wasmBytes = await readFile(wasmPath);
const localCodeHash = blake2AsHex(wasmBytes, 256);
const client = createWsClient(wsUri);
try {
  const api = client.getTypedApi(deos);
  const finalizedBlock = await client.getFinalizedBlock();
  const [authorization, runtimeVersion, liveCodeHex, submissionAuthority, vetoAssetDetails] =
    await Promise.all([
      api.view.Governance.authorized_runtime_upgrade({ at: finalizedBlock.hash }),
      api.apis.Core.version({ at: finalizedBlock.hash }),
      client._request("state_getStorage", ["0x3a636f6465", finalizedBlock.hash]),
      api.view.Governance.proposal_submission_authority(
        governanceDomainId,
        PapiEnum("L1RootAction"),
        { at: finalizedBlock.hash },
      ),
      api.view.Assets.asset_details(vetoAssetId, { at: finalizedBlock.hash }),
    ]);
  if (typeof liveCodeHex !== "string") {
    throw new Error("Finalized runtime code is unavailable");
  }
  const liveCodeHash = blake2AsHex(liveCodeHex, 256);
  const liveCodeMatchesLocalCandidate =
    liveCodeHash.toLowerCase() === localCodeHash.toLowerCase();
  const submissionAuthorityType = submissionAuthority.type;
  const vetoSupply = vetoAssetDetails?.supply ?? null;
  const strategicIngressPhase = !liveCodeMatchesLocalCandidate
    ? "different-runtime-code"
    : submissionAuthorityType !== "PrimaryEligibleSigned"
      ? "primary-eligible-ingress-unavailable"
      : vetoSupply === null || vetoSupply === 0n
        ? "protection-supply-unavailable"
        : "authorization-lifecycle-ready";
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
  let governancePlan = null;
  if (mode === "prepare-authorization") {
    const strategicPayloadBytes = hexToU8a(localCodeHash);
    const strategicPayloadHash = blake2AsHex(strategicPayloadBytes, 256);
    const stakedNativeAssetId = 0x50000000 | governanceDomainId;
    const [
      proposalStatus,
      preimageStatus,
      preimageNoteCost,
      openingFee,
      proposerNativeAssetBalance,
      proposerReceiptBalance,
      stakingPool,
      proposerSystemAccount,
    ] = await Promise.all([
      api.view.Governance.proposal_status(governanceDomainId, proposalItemId, {
        at: finalizedBlock.hash,
      }),
      api.view.Governance.payload_hash_preimage_status(strategicPayloadHash, {
        at: finalizedBlock.hash,
      }),
      api.view.Governance.payload_preimage_note_cost(strategicPayloadBytes.length, {
        at: finalizedBlock.hash,
      }),
      api.view.Governance.proposal_opening_fee(
        governanceDomainId,
        PapiEnum("L1RootAction"),
        { at: finalizedBlock.hash },
      ),
      api.view.Assets.balance_of(proposerAddress, governanceDomainId, {
        at: finalizedBlock.hash,
      }),
      api.view.Assets.balance_of(proposerAddress, stakedNativeAssetId, {
        at: finalizedBlock.hash,
      }),
      api.query.Staking.Pools.getValue(governanceDomainId, {
        at: finalizedBlock.hash,
      }),
      api.query.System.Account.getValue(proposerAddress, {
        at: finalizedBlock.hash,
      }),
    ]);
    const nativeAssetBalance = proposerNativeAssetBalance ?? 0n;
    const receiptBalance = proposerReceiptBalance ?? 0n;
    const systemFreeBalance = proposerSystemAccount.data.free;
    const hasPrimaryStake = stakingPool !== undefined && receiptBalance > 0n;
    const proposalItemAvailable = proposalStatus === undefined;
    const preimageAlreadyNoted = preimageStatus.have_preimage;
    const requiredNativeFee = (openingFee ?? 0n)
      + (preimageAlreadyNoted ? 0n : (preimageNoteCost ?? 0n));
    const creationPhase = !liveCodeMatchesLocalCandidate
      ? "different-runtime-code"
      : submissionAuthorityType !== "PrimaryEligibleSigned"
        ? "primary-eligible-ingress-unavailable"
        : !proposalItemAvailable
          ? "proposal-item-unavailable"
          : !hasPrimaryStake && nativeAssetBalance < strategicStakeAmount
            ? "insufficient-staking-asset-balance"
            : systemFreeBalance < requiredNativeFee
              ? "insufficient-native-fee-balance"
              : "ready-for-approved-proposal-creation";
    const creationCalls = [];
    if (!hasPrimaryStake) {
      creationCalls.push(await encoded(
        "establish nonzero protocol primary-track stake",
        "Signed proposer",
        api.tx.Staking.stake_native,
        { amount: strategicStakeAmount },
      ));
    }
    if (!preimageAlreadyNoted) {
      creationCalls.push(await encoded(
        "note exact strategic runtime-upgrade payload preimage",
        "Signed proposer",
        api.tx.Preimage.note_preimage,
        { bytes: strategicPayloadBytes },
      ));
    }
    creationCalls.push(await encoded(
      "create protocol L1RootAction proposal through AUTH-1",
      "Signed proposer with primary governance power",
      api.tx.Governance.submit_signed_proposal,
      {
        domain: governanceDomainId,
        item_id: proposalItemId,
        cadence_mode: PapiEnum("Ordinary"),
        payload_kind: PapiEnum("L1RootAction"),
        payload_hash: strategicPayloadHash,
      },
    ));
    const authorizationCall = strategicIngressPhase === "authorization-lifecycle-ready"
      ? await encoded(
        "cast protection-track Pass after legitimate VETO issuance",
        "Signed holder of legitimate protection power",
        api.tx.Governance.cast_vote,
        {
          domain: governanceDomainId,
          item_id: proposalItemId,
          vote: PapiEnum("Pass"),
        },
      )
      : null;
    governancePlan = {
      proposerAddress,
      proposalItemId,
      strategicStakeAmount,
      strategicPayloadBytes,
      strategicPayloadHash,
      candidateCodeHash: localCodeHash,
      creationPhase,
      creationCallsAreSubmittable:
        creationPhase === "ready-for-approved-proposal-creation",
      authorizationPhase:
        vetoSupply === null || vetoSupply === 0n
          ? "protection-supply-unavailable"
          : "requires-separately-approved-unanimous-pass",
      finalizedChecks: {
        proposalItemAvailable,
        proposalStatus: proposalStatus ?? null,
        preimageAlreadyNoted,
        preimageStatus,
        preimageNoteCost,
        openingFee,
        nativeAssetBalance,
        stakedNativeAssetId,
        receiptBalance,
        hasPrimaryStake,
        systemFreeBalance,
        requiredNativeFee,
      },
      creationCalls,
      authorizationCall,
      submissionPolicy:
        "Plan-only: this helper never signs or submits governance calls; each account action requires separate approval and finalized revalidation",
    };
  }
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
  const ingressRecommendedAction =
    strategicIngressPhase === "different-runtime-code"
      ? "Run the readiness check against a finalized network using the exact local candidate runtime"
      : strategicIngressPhase === "primary-eligible-ingress-unavailable"
        ? "The live runtime does not expose AUTH-1 for protocol L1RootAction"
        : strategicIngressPhase === "protection-supply-unavailable"
          ? "Establish legitimate protection-governance issuance before authorization; do not substitute a rehearsal fixture or privileged shortcut"
          : "AUTH-1 is reachable, but proposal submission and voting remain separately approved account actions";
  const recommendedAction = mode === "prepare-authorization"
    ? governancePlan.creationPhase === "ready-for-approved-proposal-creation"
      ? "Obtain explicit approval for the emitted proposal-creation account actions, submit them in order, wait for finality, and rerun check"
      : "Resolve the reported proposal-creation blocker before any account action"
    : authorization
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
    finalizedBlock: {
      number: finalizedBlock.number,
      hash: finalizedBlock.hash,
    },
    runtimeVersion: {
      specName: runtimeVersion.spec_name,
      specVersion: runtimeVersion.spec_version,
      transactionVersion: runtimeVersion.transaction_version,
    },
    wasmPath,
    wasmByteLength: wasmBytes.length,
    localCodeHash,
    liveCodeHash,
    liveCodeMatchesLocalCandidate,
    strategicIngress: {
      governanceDomainId,
      payloadKind: "L1RootAction",
      submissionAuthority: submissionAuthorityType,
      vetoAssetId,
      vetoSupply: vetoSupply?.toString() ?? null,
      phase: strategicIngressPhase,
      recommendedAction: ingressRecommendedAction,
    },
    governancePlan,
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
      helperSubmissionSurface: mode === "apply"
        ? "Plan-only unless --submit is provided"
        : "Read-only verifier and optional plan-only call-data emitter; no governance submission path",
    },
  };
  if (jsonOutput) {
    console.log(stringify(payload));
  } else {
    console.log(`Finalized block:             ${payload.finalizedBlock.number} ${payload.finalizedBlock.hash}`);
    console.log(`Live runtime spec version:   ${payload.runtimeVersion.specVersion}`);
    console.log(`Authorized upgrade present: ${payload.authorizedUpgrade ? "yes" : "no"}`);
    console.log(`Local WASM bytes:           ${payload.wasmByteLength}`);
    console.log(`Local code hash:            ${payload.localCodeHash}`);
    console.log(`Live code hash:             ${payload.liveCodeHash}`);
    console.log(`Candidate code live:        ${payload.liveCodeMatchesLocalCandidate ? "yes" : "no"}`);
    console.log(`Strategic ingress:          ${payload.strategicIngress.phase}`);
    console.log(`Submission authority:       ${payload.strategicIngress.submissionAuthority}`);
    console.log(`VETO issuance:              ${payload.strategicIngress.vetoSupply ?? "unavailable"}`);
    if (payload.governancePlan) {
      console.log(`Proposal creation phase:    ${payload.governancePlan.creationPhase}`);
      console.log(`Authorization phase:        ${payload.governancePlan.authorizationPhase}`);
      console.log(`Proposal payload hash:      ${payload.governancePlan.strategicPayloadHash}`);
      console.log(`Proposal creation calls:    ${payload.governancePlan.creationCalls.length}`);
      console.log(`Submission policy:          ${payload.governancePlan.submissionPolicy}`);
      console.log(stringify({
        finalizedChecks: payload.governancePlan.finalizedChecks,
        creationCalls: payload.governancePlan.creationCalls,
        authorizationCall: payload.governancePlan.authorizationCall,
      }));
    }
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
    console.log(`Ingress action:             ${payload.strategicIngress.recommendedAction}`);
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
