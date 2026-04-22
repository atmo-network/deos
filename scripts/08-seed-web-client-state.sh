#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

WS_ENDPOINT="${WS_ENDPOINT:-ws://127.0.0.1:9988}"
FOREIGN_ID="${FOREIGN_ID:-4026531841}"
INITIAL_PRICE="${INITIAL_PRICE:-1000000000000}"
SLOPE="${SLOPE:-1000000}"
MINT_AMOUNT="${MINT_AMOUNT:-50000000000000}"
LIQUIDITY_NATIVE="${LIQUIDITY_NATIVE:-5000000000000}"
LIQUIDITY_FOREIGN="${LIQUIDITY_FOREIGN:-5000000000000}"

usage() {
    cat <<'EOF'
Usage: 08-seed-web-client-state.sh [OPTIONS]

Seeds the live local parachain with the remaining minimum economic state needed for real
web-client wallet/swap testing: the current dev/local chain spec is expected to
already provide the foreign asset, router tracking, and native curve, while this
script tops up Alice's foreign balance when needed and creates/boots the
native/foreign pool with starter liquidity if it is still empty.

Options:
  -h, --help  Show this help message

Environment:
  WS_ENDPOINT=ws://127.0.0.1:9988
  FOREIGN_ID=4026531841
  INITIAL_PRICE=1000000000000
  SLOPE=1000000
  MINT_AMOUNT=50000000000000
  LIQUIDITY_NATIVE=5000000000000
  LIQUIDITY_FOREIGN=5000000000000
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
    require_directory "$PROJECT_ROOT/web-client" "web-client workspace"
    require_commands node
    log_success "Web-client state seeding prerequisites checked"
}

seed_state() {
    phase_banner "Step 2: Seed live local state"
    (
        cd "$PROJECT_ROOT/web-client"
        WS_ENDPOINT="$WS_ENDPOINT" \
        FOREIGN_ID="$FOREIGN_ID" \
        INITIAL_PRICE="$INITIAL_PRICE" \
        SLOPE="$SLOPE" \
        MINT_AMOUNT="$MINT_AMOUNT" \
        LIQUIDITY_NATIVE="$LIQUIDITY_NATIVE" \
        LIQUIDITY_FOREIGN="$LIQUIDITY_FOREIGN" \
        node --input-type=module - <<'EOF'
import { createWsClient } from 'polkadot-api/ws';
import { deos } from '@polkadot-api/descriptors';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';
import { getPolkadotSigner } from '@polkadot-api/signer';

const WS = process.env.WS_ENDPOINT;
const FOREIGN_ID = Number(process.env.FOREIGN_ID);
const INITIAL_PRICE = BigInt(process.env.INITIAL_PRICE);
const SLOPE = BigInt(process.env.SLOPE);
const MINT_AMOUNT = BigInt(process.env.MINT_AMOUNT);
const LIQUIDITY_NATIVE = BigInt(process.env.LIQUIDITY_NATIVE);
const LIQUIDITY_FOREIGN = BigInt(process.env.LIQUIDITY_FOREIGN);

function multiId(address) {
  return { type: 'Id', value: address };
}
function assetNative() {
  return { type: 'Native' };
}
function assetForeign(id = FOREIGN_ID) {
  return { type: 'Foreign', value: id };
}
function encode(value) {
  return JSON.stringify(value, (_, inner) => typeof inner === 'bigint' ? inner.toString() : inner, 2);
}

async function main() {
  await cryptoWaitReady();
  const keyring = new Keyring({ type: 'sr25519', ss58Format: 42 });
  const alice = keyring.createFromUri('//Alice', { name: 'Alice' }, 'sr25519');
  const signer = getPolkadotSigner(alice.publicKey, 'Sr25519', (input) => alice.sign(input));
  const client = createWsClient(WS);
  const api = client.getTypedApi(deos);
  const submit = async (label, tx) => {
    console.log(`\n== ${label} ==`);
    const result = await tx.signAndSubmit(signer);
    console.log(encode({ txHash: result.txHash, block: result.block, ok: result.ok }));
    return result;
  };
  const isStaleTxError = (error) =>
    typeof error === 'object' &&
    error !== null &&
    'error' in error &&
    error.error?.type === 'Invalid' &&
    error.error?.value?.type === 'Stale';
  const submitWithRetry = async (label, buildTx, attempts = 3) => {
    let lastError = null;
    for (let attempt = 1; attempt <= attempts; attempt += 1) {
      try {
        return await submit(label, buildTx());
      } catch (error) {
        lastError = error;
        if (!isStaleTxError(error) || attempt === attempts) {
          throw error;
        }
        console.warn(`Retrying ${label} after stale transaction (attempt ${attempt + 1}/${attempts})`);
      }
    }
    throw lastError;
  };
  try {
    const fin = await client.getFinalizedBlock();
    const tracked = await api.query.AxialRouter.TrackedAssets.getValue({ at: fin.hash }) ?? [];
    const curve = await api.query.TokenMintingCurve.TokenCurves.getValue(assetNative(), { at: fin.hash });
    const assetDetails = await api.view.Assets.asset_details(FOREIGN_ID, { at: fin.hash });
    const poolId = await api.query.AssetConversion.Pools.getValue([assetNative(), assetForeign()], { at: fin.hash });
    const reserves = await api.view.AssetConversion.get_reserves(assetForeign(), assetNative(), { at: fin.hash });

    if (!assetDetails) {
      throw new Error(`Foreign asset ${FOREIGN_ID} is missing. Regenerate the local chain spec with the current runtime presets before running this seed script.`);
    }
    console.log('\n== foreign asset ==\nalready exists from genesis bootstrap');

    const aliceForeign = await api.view.Assets.balance_of(alice.address, FOREIGN_ID);
    if ((aliceForeign ?? 0n) < MINT_AMOUNT) {
      await submitWithRetry('mint foreign asset to Alice', () => api.tx.Assets.mint({
        id: FOREIGN_ID,
        beneficiary: multiId(alice.address),
        amount: MINT_AMOUNT,
      }));
    } else {
      console.log('\n== foreign funding ==\nAlice already has sufficient foreign balance');
    }

    if (!tracked.some((asset) => asset.type === 'Foreign' && asset.value === FOREIGN_ID)) {
      throw new Error(`Foreign asset ${FOREIGN_ID} is not tracked by AxialRouter. Regenerate the local chain spec with the current runtime presets before running this seed script.`);
    }
    console.log('\n== tracked assets ==\nforeign asset already tracked from genesis bootstrap');

    if (!curve) {
      throw new Error('Native curve is missing. Regenerate the local chain spec with the current runtime presets before running this seed script.');
    }
    console.log('\n== native curve ==\nalready exists from genesis bootstrap');

    if (poolId == null) {
      await submitWithRetry('create native-foreign pool', () => api.tx.AssetConversion.create_pool({
        asset1: assetNative(),
        asset2: assetForeign(),
      }));
    } else {
      console.log('\n== pool ==\nalready exists');
    }

    const latest = await client.getFinalizedBlock();
    const latestReserves = await api.view.AssetConversion.get_reserves(assetForeign(), assetNative(), { at: latest.hash });
    if (!latestReserves.success || latestReserves.value[0] === 0n || latestReserves.value[1] === 0n) {
      await submitWithRetry('add native-foreign liquidity', () => api.tx.AssetConversion.add_liquidity({
        asset1: assetNative(),
        asset2: assetForeign(),
        amount1_desired: LIQUIDITY_NATIVE,
        amount2_desired: LIQUIDITY_FOREIGN,
        amount1_min: 1n,
        amount2_min: 1n,
        mint_to: alice.address,
      }));
    } else {
      console.log('\n== liquidity ==\npool already has reserves');
    }

    const after = await client.getFinalizedBlock();
    const finalTracked = await api.query.AxialRouter.TrackedAssets.getValue({ at: after.hash }) ?? [];
    const finalCurve = await api.query.TokenMintingCurve.TokenCurves.getValue(assetNative(), { at: after.hash });
    const finalReserves = await api.view.AssetConversion.get_reserves(assetForeign(), assetNative(), { at: after.hash });
    console.log('\n== final state ==');
    console.log(encode({
      block: after.number,
      tracked: finalTracked,
      hasCurve: finalCurve !== null && finalCurve !== undefined,
      reserves: finalReserves,
    }));
  } finally {
    client.destroy();
  }
}

main()
  .then(() => {
    process.exit(0);
  })
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
EOF
    )
}

main() {
    parse_args "$@"
    phase_banner "DEOS live web-client state seeding"
    log_info "Plan: ws_endpoint=$WS_ENDPOINT foreign_id=$FOREIGN_ID"
    check_prerequisites
    seed_state
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
