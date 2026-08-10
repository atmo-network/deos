#!/usr/bin/env node

/*
Domain: Live composed economic-path evidence
Owns: Finalized Router, Oracle, and Burn Actor state/event reconciliation.
Excludes: Network spawning, pool seeding, runtime upgrades, and release orchestration.
Zone: Mutating local-network assurance atom invoked by the root network harness.
*/
import { deos } from '@polkadot-api/descriptors';
import { getPolkadotSigner } from '@polkadot-api/signer';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';
import { Enum as PapiEnum } from 'polkadot-api';
import { createWsClient } from 'polkadot-api/ws';

const endpoint = process.env.DEOS_WS_ENDPOINT ?? 'ws://127.0.0.1:9988';
const foreignLiteral = process.env.DEOS_COMPOSED_FOREIGN_ID ?? '4026531841';
const amountLiteral =
  process.env.DEOS_COMPOSED_SWAP_AMOUNT ?? '250000000000000';
const timeoutLiteral = process.env.DEOS_COMPOSED_TIMEOUT_MS ?? '180000';
const burnActorLiteral = process.env.DEOS_BURN_ACTOR_ID ?? '0';

if (process.argv.includes('--help') || process.argv.includes('-h')) {
  console.log(`Usage: network-composed-path.mjs

Execute one finalized Native-to-foreign DEOS Router swap and reconcile the
resulting Router fee, Oracle publication, Burn Actor cycle, pool/balance changes,
and native burn from live finalized events and storage.

Environment:
  DEOS_WS_ENDPOINT=ws://127.0.0.1:9988
  DEOS_COMPOSED_FOREIGN_ID=4026531841
  DEOS_COMPOSED_SWAP_AMOUNT=250000000000000
  DEOS_COMPOSED_TIMEOUT_MS=180000
  DEOS_BURN_ACTOR_ID=0`);
  process.exit(0);
}
if (process.argv.length > 2)
  throw new Error(`unknown argument: ${process.argv[2]}`);

function unsignedLiteral(literal, label, { positive = false } = {}) {
  const pattern = positive ? /^[1-9][0-9]*$/ : /^(0|[1-9][0-9]*)$/;
  if (!pattern.test(literal))
    throw new Error(`${label} must be a complete unsigned integer literal`);
  return literal;
}

const foreignId = Number(unsignedLiteral(foreignLiteral, 'foreign asset id'));
const amount = BigInt(
  unsignedLiteral(amountLiteral, 'composed swap amount', { positive: true }),
);
const timeoutMs = Number(
  unsignedLiteral(timeoutLiteral, 'composed timeout', { positive: true }),
);
const burnActorId = BigInt(unsignedLiteral(burnActorLiteral, 'Burn Actor id'));
if (!Number.isSafeInteger(foreignId) || foreignId > 0xffffffff)
  throw new Error('foreign asset id must fit in u32');
if (!Number.isSafeInteger(timeoutMs))
  throw new Error('composed timeout exceeds the safe integer range');

const foreign = PapiEnum('Foreign', foreignId);
const native = PapiEnum('Native');

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

function eventParts(record) {
  const event = record?.event ?? record;
  return {
    pallet: event?.type,
    variant: event?.value?.type,
    value: event?.value?.value,
  };
}

function matchesAsset(asset, type, value) {
  return (
    asset?.type === type &&
    (value === undefined || Number(asset?.value) === value)
  );
}

function matchesFeed(feed) {
  return (
    matchesAsset(feed?.asset_in, 'Native') &&
    matchesAsset(feed?.asset_out, 'Foreign', foreignId) &&
    feed?.method?.type === 'PreExecutionSpot'
  );
}

function eventsOf(records, pallet, variant) {
  return records
    .map(eventParts)
    .filter((event) => event.pallet === pallet && event.variant === variant);
}

async function finalizedState(client, api, feed) {
  const block = await client.getFinalizedBlock();
  const [
    identity,
    actorHot,
    aliceForeign,
    burnNative,
    aliceNative,
    reserves,
    observation,
    issuance,
  ] = await Promise.all([
    api.query.Actors.ActorIdentities.getValue(burnActorId, {
      at: block.hash,
    }),
    api.query.Actors.ActorHot.getValue(burnActorId, { at: block.hash }),
    api.view.Assets.balance_of(alice.address, foreignId, { at: block.hash }),
    api.query.System.Account.getValue(burnActorAccount, { at: block.hash }),
    api.query.System.Account.getValue(alice.address, { at: block.hash }),
    api.view.AssetConversion.get_reserves(native, foreign, {
      at: block.hash,
    }),
    api.query.Oracle.Observations.getValue(feed, { at: block.hash }),
    api.query.Balances.TotalIssuance.getValue({ at: block.hash }),
  ]);
  if (!identity || !actorHot)
    throw new Error(`Burn Actor ${burnActorId} is not active`);
  if (!reserves.success)
    throw new Error('foreign/Native pool reserves are unavailable');
  return {
    block,
    identity,
    actorHot,
    aliceForeign: aliceForeign ?? 0n,
    burnNative: burnNative.data.free,
    aliceNative: aliceNative.data.free,
    reserves: reserves.value,
    observation,
    issuance,
  };
}

async function waitForActorCycle(client, api, feed, initialNonce, firstBlock) {
  const deadline = Date.now() + timeoutMs;
  const records = [];
  let lastHash;
  let state;
  while (Date.now() <= deadline) {
    const block = await client.getFinalizedBlock();
    if (block.hash !== lastHash && block.number >= firstBlock.number) {
      lastHash = block.hash;
      const blockEvents = await api.query.System.Events.getValue({
        at: block.hash,
      });
      records.push(...blockEvents);
      state = await finalizedState(client, api, feed);
      if (
        state.identity.cycle_nonce > initialNonce &&
        !state.actorHot.pending_signal &&
        state.actorHot.queue_ticket == null &&
        state.actorHot.wakeup_pointer == null
      ) {
        return { records, state };
      }
    }
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
  throw new Error(
    'Burn Actor did not reach one finalized terminal cycle state',
  );
}

await cryptoWaitReady();
const alice = signerFromUri('//Alice');
const client = createWsClient(endpoint);
let burnActorAccount;
try {
  const api = client.getTypedApi(deos);
  const discoveryBlock = await client.getFinalizedBlock();
  const [identity, feeds, nativeMinimum] = await Promise.all([
    api.query.Actors.ActorIdentities.getValue(burnActorId, {
      at: discoveryBlock.hash,
    }),
    api.query.Oracle.FeedIds.getValue({ at: discoveryBlock.hash }),
    api.constants.Balances.ExistentialDeposit({ at: discoveryBlock.hash }),
  ]);
  if (!identity) throw new Error(`Burn Actor ${burnActorId} is not registered`);
  burnActorAccount = identity.sovereign_account;
  const feed = feeds.find(matchesFeed);
  if (!feed)
    throw new Error('Native-to-foreign pre-execution Oracle feed is absent');

  const before = await finalizedState(client, api, feed);
  if (before.aliceNative < amount * 2n)
    throw new Error(`Alice Native balance ${before.aliceNative} is too low`);
  if (before.reserves[0] === 0n || before.reserves[1] === 0n)
    throw new Error('Native/foreign pool reserves must be non-zero');
  if (before.actorHot.pending_signal || before.actorHot.queue_ticket != null)
    throw new Error('Burn Actor must be idle before composed-path execution');
  if (before.burnNative !== nativeMinimum)
    throw new Error(
      `Burn Actor must start with exactly one native ED anchor: expected ${nativeMinimum}, found ${before.burnNative}`,
    );
  if (before.observation != null)
    throw new Error(
      'composed-path proof requires a fresh topology with an uninitialized exact Oracle feed',
    );

  const finalized = await finalizedTransaction(
    api.tx.DeosRouter.swap({
      from: native,
      to: foreign,
      amount_in: amount,
      min_amount_out: 1n,
      recipient: alice.address,
      deadline: before.block.number + 100,
    }).signSubmitAndWatch(alice.signer),
  );
  const { records, state: after } = await waitForActorCycle(
    client,
    api,
    feed,
    before.identity.cycle_nonce,
    finalized.block,
  );

  const routerSwaps = eventsOf(records, 'DeosRouter', 'SwapExecuted');
  const userSwaps = routerSwaps.filter(
    (event) => event.value?.who === alice.address,
  );
  const fees = eventsOf(records, 'DeosRouter', 'FeeCollected').filter(
    (event) => event.value?.source === alice.address,
  );
  const oracleEvents = records
    .map(eventParts)
    .filter(
      (event) =>
        event.pallet === 'Oracle' &&
        ['ObservationPublished', 'ObservationRefreshed'].includes(
          event.variant,
        ) &&
        matchesFeed(event.value?.feed),
    );
  const cycles = eventsOf(records, 'Actors', 'CycleSummary').filter(
    (event) => event.value?.actor_id === burnActorId,
  );
  const burns = eventsOf(records, 'Actors', 'BurnExecuted').filter(
    (event) => event.value?.actor_id === burnActorId,
  );

  if (userSwaps.length !== 1)
    throw new Error('expected exactly one user Router swap');
  if (fees.length !== 1)
    throw new Error('expected exactly one user Router fee');
  if (
    oracleEvents.length !== 1 ||
    oracleEvents[0].variant !== 'ObservationPublished'
  )
    throw new Error('expected exactly one first Oracle publication');
  if (cycles.length !== 1 || cycles[0].value?.result?.type !== 'Completed')
    throw new Error('expected exactly one completed Burn Actor cycle');
  if (burns.length !== 1)
    throw new Error('expected exactly one Actors native burn');
  if (after.identity.cycle_nonce !== before.identity.cycle_nonce + 1n)
    throw new Error(
      'Burn Actor cycle nonce proves duplicate or missing execution',
    );
  const outcome = userSwaps[0].value.outcome;
  const fee = fees[0].value.amount;
  const burned = burns[0].value.amount;
  if (after.aliceForeign - before.aliceForeign !== outcome.recipient_amount_out)
    throw new Error('Alice foreign balance delta disagrees with Router output');
  if (before.aliceNative - after.aliceNative <= amount)
    throw new Error('Alice Native balance did not include swap input and fees');
  if (after.burnNative !== nativeMinimum)
    throw new Error('Burn Actor did not preserve exactly one native ED anchor');
  if (burned !== fee)
    throw new Error(
      `Burn Actor burn ${burned} disagrees with routed fee ${fee}`,
    );
  if (after.reserves[0] - before.reserves[0] !== outcome.routed_amount_in)
    throw new Error('Native reserve delta disagrees with routed Router input');
  if (before.reserves[1] - after.reserves[1] !== outcome.recipient_amount_out)
    throw new Error('foreign reserve delta disagrees with Router output');
  if (!after.observation || after.observation.revision !== 1n)
    throw new Error('first Oracle publication did not establish revision one');
  if (before.issuance - after.issuance !== burned)
    throw new Error('native issuance delta disagrees with BurnExecuted');

  console.log(
    JSON.stringify({
      endpoint,
      tx_hash: finalized.txHash,
      finalized_tx_block: finalized.block.number,
      finalized_terminal_block: after.block.number,
      swap_amount: amount.toString(),
      oracle_revision_before: (before.observation?.revision ?? 0n).toString(),
      oracle_revision_after: after.observation.revision.toString(),
      actor_cycle_nonce_before: before.identity.cycle_nonce.toString(),
      actor_cycle_nonce_after: after.identity.cycle_nonce.toString(),
      router_fee: fee.toString(),
      native_burned: burned.toString(),
      native_anchor: after.burnNative.toString(),
      native_issuance_delta: (before.issuance - after.issuance).toString(),
    }),
  );
} finally {
  client.destroy();
}
