#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

MODE="${MODE:-check}"
WS_URI="${WS_URI:-ws://127.0.0.1:9988}"
NATIVE_STAKING_ASSET_ID="${NATIVE_STAKING_ASSET_ID:-0}"
NATIVE_STAKING_LP_FARMER_AAA_ID="${NATIVE_STAKING_LP_FARMER_AAA_ID:-14}"
OPERATOR_ADDRESS="${OPERATOR_ADDRESS:-}"
STAKE_AMOUNT="${STAKE_AMOUNT:-5000000000000}"
LIQUIDITY_NATIVE="${LIQUIDITY_NATIVE:-5000000000000}"
LIQUIDITY_STAKED="${LIQUIDITY_STAKED:-5000000000000}"
MIN_NATIVE="${MIN_NATIVE:-1}"
MIN_STAKED="${MIN_STAKED:-1}"
JSON_OUTPUT="${JSON_OUTPUT:-0}"
WEB_CLIENT_DIR="$PROJECT_ROOT/web-client"

usage() {
    cat <<'EOF'
Usage: bootstrap-native-staking-local.sh <check|prepare-calls> [OPTIONS]

Plan/read-only tooling for the canonical local NTVE/stNTVE staking-pool bootstrap.
It never signs or submits transactions.

Subcommands:
  check            Check staking assets, native pool, AMM liquidity, and LP Farmer AAA readiness
  prepare-calls    Emit the next Root/governance or signed operator call data needed for bootstrap

Options:
  --ws URI                  WebSocket endpoint (default: ws://127.0.0.1:9988)
  --native-asset-id ID      Local native staking asset id (default: 0)
  --aaa-id ID               Native Staking LP Farmer AAA id for check mode (default: 14)
  --operator-address SS58   Operator account for prepare-calls mode
  --stake-amount AMOUNT     NTVE amount to stake into stNTVE before liquidity
  --liquidity-native AMOUNT NTVE side for initial add_liquidity
  --liquidity-staked AMOUNT stNTVE side for initial add_liquidity
  --min-native AMOUNT       Minimum NTVE accepted by add_liquidity
  --min-staked AMOUNT       Minimum stNTVE accepted by add_liquidity
  --json                    Emit machine-readable JSON
  -h, --help                Show this help message

Environment mirrors the option names in uppercase. MODE=check|prepare-calls is also supported.
EOF
}

parse_args() {
    if [[ $# -gt 0 && "$1" != -* ]]; then
        MODE="$1"
        shift
    fi
    case "$MODE" in
        check|prepare-calls) ;;
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
            --native-asset-id)
                [[ $# -ge 2 ]] || { log_error "Missing value for --native-asset-id"; usage; exit 1; }
                NATIVE_STAKING_ASSET_ID="$2"
                shift
                ;;
            --aaa-id)
                [[ $# -ge 2 ]] || { log_error "Missing value for --aaa-id"; usage; exit 1; }
                NATIVE_STAKING_LP_FARMER_AAA_ID="$2"
                shift
                ;;
            --operator-address)
                [[ $# -ge 2 ]] || { log_error "Missing value for --operator-address"; usage; exit 1; }
                OPERATOR_ADDRESS="$2"
                shift
                ;;
            --stake-amount)
                [[ $# -ge 2 ]] || { log_error "Missing value for --stake-amount"; usage; exit 1; }
                STAKE_AMOUNT="$2"
                shift
                ;;
            --liquidity-native)
                [[ $# -ge 2 ]] || { log_error "Missing value for --liquidity-native"; usage; exit 1; }
                LIQUIDITY_NATIVE="$2"
                shift
                ;;
            --liquidity-staked)
                [[ $# -ge 2 ]] || { log_error "Missing value for --liquidity-staked"; usage; exit 1; }
                LIQUIDITY_STAKED="$2"
                shift
                ;;
            --min-native)
                [[ $# -ge 2 ]] || { log_error "Missing value for --min-native"; usage; exit 1; }
                MIN_NATIVE="$2"
                shift
                ;;
            --min-staked)
                [[ $# -ge 2 ]] || { log_error "Missing value for --min-staked"; usage; exit 1; }
                MIN_STAKED="$2"
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

require_uint() {
    local value="$1"
    local label="$2"
    if [[ ! "$value" =~ ^[0-9]+$ ]]; then
        log_error "$label must be a non-negative integer: $value"
        exit 1
    fi
}

check_prerequisites() {
    if [[ "$JSON_OUTPUT" != "1" ]]; then
        phase_banner "Step 1: Prerequisites"
    fi
    require_directory "$WEB_CLIENT_DIR" "web-client workspace"
    require_commands node
    require_uint "$NATIVE_STAKING_ASSET_ID" "Native staking asset id"
    require_uint "$NATIVE_STAKING_LP_FARMER_AAA_ID" "AAA id"
    if [[ "$MODE" == "prepare-calls" ]]; then
        require_uint "$STAKE_AMOUNT" "Stake amount"
        require_uint "$LIQUIDITY_NATIVE" "Liquidity native amount"
        require_uint "$LIQUIDITY_STAKED" "Liquidity staked amount"
        require_uint "$MIN_NATIVE" "Minimum native amount"
        require_uint "$MIN_STAKED" "Minimum staked amount"
        if [[ -z "$OPERATOR_ADDRESS" ]]; then
            log_error "--operator-address is required for prepare-calls mode"
            exit 1
        fi
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
    if [[ "$JSON_OUTPUT" != "1" ]]; then
        log_success "Native staking bootstrap prerequisites satisfied"
    fi
}

print_plan() {
    if [[ "$JSON_OUTPUT" == "1" ]]; then
        return 0
    fi
    phase_banner "Step 2: Native staking bootstrap plan"
    echo "  Mode:            $MODE"
    echo "  WS URI:          $WS_URI"
    echo "  Native asset id: $NATIVE_STAKING_ASSET_ID"
    if [[ "$MODE" == "check" ]]; then
        echo "  LP farmer AAA:   $NATIVE_STAKING_LP_FARMER_AAA_ID"
    else
        echo "  Operator:        $OPERATOR_ADDRESS"
        echo "  Stake amount:    $STAKE_AMOUNT"
        echo "  Liquidity:       $LIQUIDITY_NATIVE NTVE / $LIQUIDITY_STAKED stNTVE"
        echo "  Minimums:        $MIN_NATIVE NTVE / $MIN_STAKED stNTVE"
    fi
    echo "  Output:          $([[ "$JSON_OUTPUT" == "1" ]] && echo JSON || echo human)"
}

run_node_flow() {
    if [[ "$JSON_OUTPUT" != "1" ]]; then
        phase_banner "Step 3: Native staking bootstrap $MODE"
    fi
    (
        cd "$WEB_CLIENT_DIR"
        MODE="$MODE" \
        WS_URI="$WS_URI" \
        NATIVE_STAKING_ASSET_ID="$NATIVE_STAKING_ASSET_ID" \
        NATIVE_STAKING_LP_FARMER_AAA_ID="$NATIVE_STAKING_LP_FARMER_AAA_ID" \
        OPERATOR_ADDRESS="$OPERATOR_ADDRESS" \
        STAKE_AMOUNT="$STAKE_AMOUNT" \
        LIQUIDITY_NATIVE="$LIQUIDITY_NATIVE" \
        LIQUIDITY_STAKED="$LIQUIDITY_STAKED" \
        MIN_NATIVE="$MIN_NATIVE" \
        MIN_STAKED="$MIN_STAKED" \
        JSON_OUTPUT="$JSON_OUTPUT" \
        node --input-type=module <<'EOF'
import { u8aToHex } from "@polkadot/util";
import { createWsClient } from "polkadot-api/ws";
import { deos } from "@polkadot-api/descriptors";

function envUnsignedLiteral(name) {
    const value = process.env[name];
    if (!/^\d+$/.test(value ?? "")) {
        throw new Error(`${name} must be an unsigned integer literal`);
    }
    return value;
}

function parseEnvU32(name) {
    const parsed = Number(envUnsignedLiteral(name));
    if (!Number.isSafeInteger(parsed) || parsed < 0 || parsed > 0xffffffff) {
        throw new Error(`${name} must fit in u32`);
    }
    return parsed;
}

function parseEnvBigUint(name) {
    return BigInt(envUnsignedLiteral(name));
}

const mode = process.env.MODE;
const wsUri = process.env.WS_URI;
const nativeAssetId = parseEnvU32("NATIVE_STAKING_ASSET_ID");
const aaaId = parseEnvBigUint("NATIVE_STAKING_LP_FARMER_AAA_ID");
const operatorAddress = process.env.OPERATOR_ADDRESS;
const stakeAmount = parseEnvBigUint("STAKE_AMOUNT");
const liquidityNative = parseEnvBigUint("LIQUIDITY_NATIVE");
const liquidityStaked = parseEnvBigUint("LIQUIDITY_STAKED");
const minNative = parseEnvBigUint("MIN_NATIVE");
const minStaked = parseEnvBigUint("MIN_STAKED");
const jsonOutput = process.env.JSON_OUTPUT === "1";
const stakedNativeAssetId = 0x50000000 | nativeAssetId;
const client = createWsClient(wsUri);

function stringify(value) {
  return JSON.stringify(value, (_, inner) => typeof inner === "bigint" ? inner.toString() : inner, 2);
}

function assetLocal(id) {
  return { type: "Local", value: id };
}

async function encoded(label, authority, tx, params) {
  const data = await tx(params).getEncodedData();
  return { label, authority, params, callData: u8aToHex(data), callDataByteLength: data.length };
}

function checkPhase(checks) {
  if (!checks.nativeAssetExists || !checks.stakedAssetExists || !checks.exchangeRateAvailable) return "missing-staking-assets";
  if (!checks.nativeStakingPoolExists) return "missing-ntve-stntve-pool";
  if (!checks.nativeStakingPoolHasLiquidity) return "empty-ntve-stntve-pool";
  if (!checks.lpFarmerAaaExists) return "missing-native-staking-lp-farmer-aaa";
  return "ready-for-guarded-aaa-activation";
}

function checkRecommendation(phase) {
  switch (phase) {
    case "missing-staking-assets": return "Regenerate the chain spec with current presets or run staking asset registration before creating the pool";
    case "missing-ntve-stntve-pool": return "Create the canonical Local(NTVE)/Local(stNTVE) Asset Conversion pool";
    case "empty-ntve-stntve-pool": return "Seed balanced initial NTVE/stNTVE liquidity before enabling dependent flows";
    case "missing-native-staking-lp-farmer-aaa": return "Ensure genesis/system AAA configuration includes the Native Staking LP Farmer skeleton before activation";
    default: return "Run the guarded Native Staking LP Farmer activation path if it is not active yet; otherwise bootstrap is ready";
  }
}

function preparePhase(checks) {
  if (!checks.nativeAssetExists) return "missing-native-staking-asset";
  if (!checks.exchangeRateAvailable || !checks.stakedAssetExists) return "needs-staking-admin-registration";
  if (!checks.operatorHasStakedLiquidity) return "needs-operator-stntve-acquisition";
  if (!checks.nativeStakingPoolExists) return "needs-operator-pool-create";
  if (!checks.nativeStakingPoolHasLiquidity) return "needs-operator-liquidity";
  return "ntve-stntve-pool-ready";
}

function prepareRecommendation(phase) {
  switch (phase) {
    case "missing-native-staking-asset": return "Create/register the local NTVE asset before staking bootstrap can continue";
    case "needs-staking-admin-registration": return "Submit the emitted staking admin call through Root/governance, then rerun this helper";
    case "needs-operator-stntve-acquisition": return "Submit the emitted operator stake_native call, then rerun this helper after finality";
    case "needs-operator-pool-create": return "Submit the emitted AssetConversion.create_pool call, then rerun this helper after finality";
    case "needs-operator-liquidity": return "Submit the emitted AssetConversion.add_liquidity call, then run check mode";
    default: return "The canonical pool is non-empty; run check mode and then the guarded Native Staking LP Farmer activation path if needed";
  }
}

try {
  const api = client.getTypedApi(deos);
  const block = await client.getFinalizedBlock();
  const nativeAssetDetails = await api.view.Assets.asset_details(nativeAssetId, { at: block.hash });
  const stakedAssetDetails = await api.view.Assets.asset_details(stakedNativeAssetId, { at: block.hash });
  const exchangeRate = await api.view.Staking.native_staking_exchange_rate({ at: block.hash });
  const pool = await api.view.Staking.native_staking_liquidity_pool({ at: block.hash });
  if (mode === "check") {
    const lpFarmerAaa = await api.query.AAA.AaaInstances.getValue(aaaId, { at: block.hash });
    const lpFarmerReadiness = await api.query.AAA.AaaReadiness.getValue(aaaId, { at: block.hash });
    const checks = {
      nativeAssetExists: nativeAssetDetails != null,
      stakedAssetExists: stakedAssetDetails != null,
      exchangeRateAvailable: exchangeRate != null,
      nativeStakingPoolExists: pool != null,
      nativeStakingPoolHasLiquidity: pool != null && pool.reserve_native > 0n && pool.reserve_staked > 0n && pool.lp_total_issuance > 0n,
      lpFarmerAaaExists: lpFarmerAaa != null,
    };
    const phase = checkPhase(checks);
    const payload = { wsUri, block: block.number, nativeAssetId, stakedNativeAssetId, aaaId: Number(aaaId), phase, recommendedAction: checkRecommendation(phase), checks, exchangeRate, pool, lpFarmerReadiness };
    if (jsonOutput) console.log(stringify(payload));
    else {
      console.log(`Phase: ${payload.phase}`);
      console.log(`Recommended action: ${payload.recommendedAction}`);
      console.log(stringify({ block: payload.block, nativeAssetId, stakedNativeAssetId, aaaId: payload.aaaId, checks, exchangeRate, pool, lpFarmerReadiness }));
    }
  } else {
    const operatorStakedBalance = await api.view.Assets.balance_of(operatorAddress, stakedNativeAssetId, { at: block.hash }) ?? 0n;
    const checks = {
      nativeAssetExists: nativeAssetDetails != null,
      stakedAssetExists: stakedAssetDetails != null,
      exchangeRateAvailable: exchangeRate != null,
      operatorHasStakedLiquidity: operatorStakedBalance >= liquidityStaked,
      nativeStakingPoolExists: pool != null,
      nativeStakingPoolHasLiquidity: pool != null && pool.reserve_native > 0n && pool.reserve_staked > 0n && pool.lp_total_issuance > 0n,
    };
    const calls = [];
    if (checks.nativeAssetExists && (!checks.exchangeRateAvailable || !checks.stakedAssetExists)) {
      calls.push(await encoded(
        checks.exchangeRateAvailable ? "initialize stNTVE receipt asset" : "register native staking asset",
        "Root/governance staking AdminOrigin",
        checks.exchangeRateAvailable ? api.tx.Staking.initialize_staked_asset : api.tx.Staking.register_staking_asset,
        { asset_id: nativeAssetId },
      ));
    }
    if (checks.stakedAssetExists && !checks.operatorHasStakedLiquidity) {
      calls.push(await encoded("stake NTVE into stNTVE for bootstrap liquidity", "Signed operator", api.tx.Staking.stake_native, { amount: stakeAmount }));
    }
    if (checks.stakedAssetExists && !checks.nativeStakingPoolExists) {
      calls.push(await encoded("create canonical NTVE/stNTVE pool", "Signed operator", api.tx.AssetConversion.create_pool, { asset1: assetLocal(nativeAssetId), asset2: assetLocal(stakedNativeAssetId) }));
    }
    if (checks.stakedAssetExists && checks.operatorHasStakedLiquidity && !checks.nativeStakingPoolHasLiquidity) {
      calls.push(await encoded("seed canonical NTVE/stNTVE liquidity", "Signed operator", api.tx.AssetConversion.add_liquidity, {
        asset1: assetLocal(nativeAssetId), asset2: assetLocal(stakedNativeAssetId), amount1_desired: liquidityNative, amount2_desired: liquidityStaked, amount1_min: minNative, amount2_min: minStaked, mint_to: operatorAddress,
      }));
    }
    const phase = preparePhase(checks);
    const payload = { wsUri, block: block.number, nativeAssetId, stakedNativeAssetId, operatorAddress, requestedAmounts: { stakeAmount, liquidityNative, liquidityStaked, minNative, minStaked }, phase, recommendedAction: prepareRecommendation(phase), checks, exchangeRate, pool, operatorStakedBalance, calls, submissionPolicy: "Plan-only: submit emitted calls through the stated authority; this helper never signs or submits" };
    if (jsonOutput) console.log(stringify(payload));
    else {
      console.log(`Phase: ${payload.phase}`);
      console.log(`Recommended action: ${payload.recommendedAction}`);
      console.log(`Submission policy: ${payload.submissionPolicy}`);
      console.log(stringify({ block: payload.block, nativeAssetId, stakedNativeAssetId, operatorAddress, checks, exchangeRate, pool, operatorStakedBalance, calls }));
    }
  }
} finally {
  client.destroy();
}
EOF
    )
}

main() {
    parse_args "$@"
    if [[ "$JSON_OUTPUT" != "1" ]]; then
        phase_banner "DEOS native staking bootstrap local tool"
    fi
    check_prerequisites
    print_plan
    run_node_flow
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
