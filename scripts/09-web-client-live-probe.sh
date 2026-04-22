#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

WS_ENDPOINT="${WS_ENDPOINT:-ws://127.0.0.1:9988}"
AUTO_SEED="${AUTO_SEED:-1}"
TRANSFER_AMOUNT="${TRANSFER_AMOUNT:-123456789}"
FOREIGN_TRANSFER_AMOUNT="${FOREIGN_TRANSFER_AMOUNT:-123456789}"
SWAP_AMOUNT="${SWAP_AMOUNT:-2000000000000}"

usage() {
    cat <<'EOF'
Usage: 09-web-client-live-probe.sh [OPTIONS]

Runs live wallet/swap probes against the local parachain RPC using the same
Alice/Bob dev identities and signing stack as the web-client.

Options:
  --no-seed   Skip the local state seeding step
  -h, --help  Show this help message

Environment:
  WS_ENDPOINT=ws://127.0.0.1:9988
  AUTO_SEED=1|0
  TRANSFER_AMOUNT=123456789
  FOREIGN_TRANSFER_AMOUNT=123456789
  SWAP_AMOUNT=2000000000000
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --no-seed)
                AUTO_SEED=0
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
    require_directory "$PROJECT_ROOT/web-client" "web-client workspace"
    require_local_script "08-seed-web-client-state.sh"
    require_commands node
    log_success "Live probe prerequisites checked"
}

seed_if_requested() {
    if (( AUTO_SEED == 0 )); then
        log_warning "Skipping local state seeding"
        return
    fi
    phase_banner "Step 2: Seed local web-client state"
    WS_ENDPOINT="$WS_ENDPOINT" "$SCRIPT_DIR/08-seed-web-client-state.sh"
}

run_live_probe() {
    phase_banner "Step 3: Run live transfer + swap probes"
    (
        cd "$PROJECT_ROOT/web-client"
        WS_ENDPOINT="$WS_ENDPOINT" \
        TRANSFER_AMOUNT="$TRANSFER_AMOUNT" \
        FOREIGN_TRANSFER_AMOUNT="$FOREIGN_TRANSFER_AMOUNT" \
        SWAP_AMOUNT="$SWAP_AMOUNT" \
        node --input-type=module - <<'EOF'
import { createWsClient } from 'polkadot-api/ws';
import { deos } from '@polkadot-api/descriptors';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';
import { getPolkadotSigner } from '@polkadot-api/signer';

const WS = process.env.WS_ENDPOINT;
const TRANSFER_AMOUNT = BigInt(process.env.TRANSFER_AMOUNT);
const FOREIGN_TRANSFER_AMOUNT = BigInt(process.env.FOREIGN_TRANSFER_AMOUNT);
const SWAP_AMOUNT = BigInt(process.env.SWAP_AMOUNT);
const FOREIGN_ID = 0xf0000001;

function encode(value) {
  return JSON.stringify(value, (_, inner) => typeof inner === 'bigint' ? inner.toString() : inner, 2);
}

async function main() {
  await cryptoWaitReady();
  const keyring = new Keyring({ type: 'sr25519', ss58Format: 42 });
  const alice = keyring.createFromUri('//Alice', { name: 'Alice' }, 'sr25519');
  const bob = keyring.createFromUri('//Bob', { name: 'Bob' }, 'sr25519');
  const signer = getPolkadotSigner(alice.publicKey, 'Sr25519', (input) => alice.sign(input));
  const client = createWsClient(WS);
  const api = client.getTypedApi(deos);
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
        return await buildTx().signAndSubmit(signer);
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
    console.log('\n== live transfer probe ==');
    const beforeAlice = (await api.query.System.Account.getValue(alice.address))?.data?.free ?? 0n;
    const beforeBob = (await api.query.System.Account.getValue(bob.address))?.data?.free ?? 0n;
    console.log(encode({ beforeAlice, beforeBob }));
    const transfer = await submitWithRetry('live native transfer probe', () => api.tx.Balances.transfer_keep_alive({
      dest: { type: 'Id', value: bob.address },
      value: TRANSFER_AMOUNT,
    }));
    console.log(encode({ transferTxHash: transfer.txHash, transferOk: transfer.ok, transferBlock: transfer.block }));

    console.log('\n== live tracked-asset transfer probe ==');
    const beforeAliceForeign = (await api.view.Assets.balance_of(alice.address, FOREIGN_ID)) ?? 0n;
    const beforeBobForeign = (await api.view.Assets.balance_of(bob.address, FOREIGN_ID)) ?? 0n;
    console.log(encode({ beforeAliceForeign, beforeBobForeign }));
    const foreignTransfer = await submitWithRetry('live tracked-asset transfer probe', () => api.tx.Assets.transfer_keep_alive({
      id: FOREIGN_ID,
      target: { type: 'Id', value: bob.address },
      amount: FOREIGN_TRANSFER_AMOUNT,
    }));
    console.log(encode({ foreignTransferTxHash: foreignTransfer.txHash, foreignTransferOk: foreignTransfer.ok, foreignTransferBlock: foreignTransfer.block }));

    console.log('\n== live swap probe ==');
    const fin = await client.getFinalizedBlock();
    const quote = await api.view.AxialRouter.quote_exact_input(
      alice.address,
      { type: 'Foreign', value: FOREIGN_ID },
      { type: 'Native' },
      SWAP_AMOUNT,
      { at: fin.hash },
    );
    console.log(encode({ quote }));
    if (!quote || !quote.success) {
      throw new Error('Swap quote failed during live probe');
    }
    const swap = await submitWithRetry('live swap probe', () => api.tx.AxialRouter.swap({
      from: { type: 'Foreign', value: FOREIGN_ID },
      to: { type: 'Native' },
      amount_in: SWAP_AMOUNT,
      min_amount_out: (quote.value.amount_out * 99n) / 100n,
      recipient: alice.address,
      deadline: fin.number + 50,
    }));
    console.log(encode({ swapTxHash: swap.txHash, swapOk: swap.ok, swapBlock: swap.block, swapEvents: swap.events.length }));
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
    phase_banner "DEOS live web-client probe"
    log_info "Plan: ws_endpoint=$WS_ENDPOINT auto_seed=$AUTO_SEED transfer_amount=$TRANSFER_AMOUNT foreign_transfer_amount=$FOREIGN_TRANSFER_AMOUNT swap_amount=$SWAP_AMOUNT"
    check_prerequisites
    seed_if_requested
    run_live_probe
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
