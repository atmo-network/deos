#!/usr/bin/env node

import { deos } from '@polkadot-api/descriptors';
import { getPolkadotSigner } from '@polkadot-api/signer';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';
import { Enum as PapiEnum } from 'polkadot-api';
import { createWsClient } from 'polkadot-api/ws';

const endpoint = process.env.DEOS_WS_ENDPOINT ?? 'ws://127.0.0.1:9988';
const amountLiteral = process.env.DEOS_E2E_TRANSFER_AMOUNT ?? '1000000000';
const timeoutLiteral = process.env.DEOS_E2E_TIMEOUT_MS ?? '120000';
if (
  !/^[1-9][0-9]*$/.test(amountLiteral) ||
  !/^[1-9][0-9]*$/.test(timeoutLiteral)
) {
  throw new Error(
    'E2E amount and timeout must be complete positive integer literals',
  );
}
const amount = BigInt(amountLiteral);
const timeoutMs = Number(timeoutLiteral);

if (process.argv.includes('--help') || process.argv.includes('-h')) {
  console.log(`Usage: network-e2e.mjs

Submit one signed native transfer through a running DEOS node, require finalized
success and a Balances.Transfer event, then verify the finalized recipient balance.

Environment:
  DEOS_WS_ENDPOINT=ws://127.0.0.1:9988
  DEOS_E2E_TRANSFER_AMOUNT=1000000000
  DEOS_E2E_TIMEOUT_MS=120000`);
  process.exit(0);
}
if (process.argv.length > 2)
  throw new Error(`unknown argument: ${process.argv[2]}`);
if (!Number.isSafeInteger(timeoutMs))
  throw new Error('E2E timeout exceeds the safe integer range');

function signerFromUri(uri) {
  const keyring = new Keyring({ type: 'sr25519', ss58Format: 42 });
  const pair = keyring.createFromUri(uri, {}, 'sr25519');
  return {
    address: pair.address,
    signer: getPolkadotSigner(pair.publicKey, 'Sr25519', (input) =>
      pair.sign(input),
    ),
  };
}

function transferEvent(events) {
  return events.some((record) => {
    const event = record?.event ?? record;
    return event?.type === 'Balances' && event?.value?.type === 'Transfer';
  });
}

function finalizedTransaction(watcher) {
  return new Promise((resolve, reject) => {
    let subscription;
    const timer = setTimeout(() => {
      subscription?.unsubscribe();
      reject(new Error(`transaction did not finalize within ${timeoutMs}ms`));
    }, timeoutMs);
    subscription = watcher.subscribe({
      next(event) {
        if (event.type !== 'finalized') return;
        clearTimeout(timer);
        subscription?.unsubscribe();
        if (!event.ok) {
          reject(
            new Error(
              `finalized dispatch failed: ${JSON.stringify(event.dispatchError)}`,
            ),
          );
          return;
        }
        if (!transferEvent(event.events)) {
          reject(new Error('finalized transaction omitted Balances.Transfer'));
          return;
        }
        resolve(event);
      },
      error(error) {
        clearTimeout(timer);
        subscription?.unsubscribe();
        reject(error);
      },
    });
  });
}

await cryptoWaitReady();
const alice = signerFromUri('//Alice');
const bob = signerFromUri('//Bob');
const client = createWsClient(endpoint);
try {
  const api = client.getTypedApi(deos);
  const beforeBlock = await client.getFinalizedBlock();
  const before = await api.query.System.Account.getValue(bob.address, {
    at: beforeBlock.hash,
  });
  const finalized = await finalizedTransaction(
    api.tx.Balances.transfer_keep_alive({
      dest: PapiEnum('Id', bob.address),
      value: amount,
    }).signSubmitAndWatch(alice.signer),
  );
  const after = await api.query.System.Account.getValue(bob.address, {
    at: finalized.block.hash,
  });
  if (after.data.free - before.data.free !== amount) {
    throw new Error(
      `finalized recipient delta mismatch: expected ${amount}, found ${after.data.free - before.data.free}`,
    );
  }
  console.log(
    JSON.stringify({
      endpoint,
      tx_hash: finalized.txHash,
      finalized_block: finalized.block.number,
      recipient_delta: amount.toString(),
    }),
  );
} finally {
  client.destroy();
}
