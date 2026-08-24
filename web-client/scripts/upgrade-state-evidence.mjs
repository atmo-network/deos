#!/usr/bin/env node

/*
Domain: Runtime-upgrade state evidence
Owns: Finalized pre/post upgrade snapshots and preservation comparison.
Excludes: Baseline builds, governance authorization, code relay, and network lifecycle.
Zone: Read-only live assurance atom invoked by the authorized-upgrade helper.
*/
import { deos } from '@polkadot-api/descriptors';
import { blake2AsHex } from '@polkadot/util-crypto';
import { readFile, writeFile } from 'node:fs/promises';
import { Enum as PapiEnum } from 'polkadot-api';
import { createWsClient } from 'polkadot-api/ws';

if (process.argv.includes('--help') || process.argv.includes('-h')) {
  console.log(`Usage: upgrade-state-evidence.mjs

Internal finalized-state capture/verification atom. Invoke through:
  authorized-upgrade-local.sh snapshot|verify --state PATH`);
  process.exit(0);
}
if (process.argv.length > 2)
  throw new Error(`unknown argument: ${process.argv[2]}`);

const mode = process.env.MODE;
const endpoint = process.env.WS_URI;
const statePath = process.env.UPGRADE_STATE_PATH;
const wasmPath = process.env.WASM_PATH;
const foreignLiteral = process.env.UPGRADE_FOREIGN_ID ?? '4026531841';
const sourceSpecLiteral = process.env.UPGRADE_SOURCE_SPEC_VERSION ?? '1';
const targetSpecLiteral = process.env.UPGRADE_TARGET_SPEC_VERSION ?? '2';
const burnActorLiteral = process.env.UPGRADE_BURN_ACTOR_ID ?? '0';

function u32(literal, label) {
  if (!/^(0|[1-9][0-9]*)$/.test(literal))
    throw new Error(`${label} must be a complete unsigned integer literal`);
  const value = Number(literal);
  if (!Number.isSafeInteger(value) || value > 0xffffffff)
    throw new Error(`${label} must fit in u32`);
  return value;
}

const foreignId = u32(foreignLiteral, 'foreign asset id');
const sourceSpecVersion = u32(sourceSpecLiteral, 'source spec version');
const targetSpecVersion = u32(targetSpecLiteral, 'target spec version');
const burnActorId = BigInt(u32(burnActorLiteral, 'Burn Actor id').toString());
const native = PapiEnum('Native');
const foreign = PapiEnum('Foreign', foreignId);
const codeStorageKey = '0x3a636f6465';

function jsonValue(value) {
  return JSON.parse(
    JSON.stringify(value, (_, inner) =>
      typeof inner === 'bigint' ? inner.toString() : inner,
    ),
  );
}

function same(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function assetMatches(asset, type, value) {
  return (
    asset?.type === type &&
    (value === undefined || Number(asset.value) === value)
  );
}

function feedMatches(feed) {
  return (
    assetMatches(feed?.asset_in, 'Native') &&
    assetMatches(feed?.asset_out, 'Foreign', foreignId) &&
    feed?.method?.type === 'PreExecutionSpot'
  );
}

async function chainSnapshot(client, api) {
  const block = await client.getFinalizedBlock();
  const [version, codeHex, feeds, actorIdentity] = await Promise.all([
    api.apis.Core.version({ at: block.hash }),
    client._request('state_getStorage', [codeStorageKey, block.hash]),
    api.query.Oracle.FeedIds.getValue({ at: block.hash }),
    api.query.Actors.ActorIdentities.getValue(burnActorId, {
      at: block.hash,
    }),
  ]);
  if (typeof codeHex !== 'string')
    throw new Error('finalized runtime code is unavailable');
  if (!actorIdentity) throw new Error('Burn Actor identity is unavailable');
  const feed = feeds.find(feedMatches);
  if (!feed)
    throw new Error('Native-to-foreign pre-execution Oracle feed is absent');
  const burnAccount = actorIdentity.sovereign_account;
  const [
    burnNative,
    burnForeign,
    assetDetails,
    pool,
    reserves,
    lpPairs,
    observation,
    actorHot,
    actorContractHead,
    actorFunding,
  ] = await Promise.all([
    api.query.System.Account.getValue(burnAccount, { at: block.hash }),
    api.view.Assets.balance_of(burnAccount, foreignId, { at: block.hash }),
    api.view.Assets.asset_details(foreignId, { at: block.hash }),
    api.query.AssetConversion.Pools.getValue([native, foreign], {
      at: block.hash,
    }),
    api.view.AssetConversion.get_reserves(native, foreign, { at: block.hash }),
    api.query.DeosRouter.LpPairByTokenId.getValue({ at: block.hash }),
    api.query.Oracle.Observations.getValue(feed, { at: block.hash }),
    api.query.Actors.ActorHot.getValue(burnActorId, { at: block.hash }),
    api.query.Actors.ActorContractHead.getValue(burnActorId, {
      at: block.hash,
    }),
    api.query.Actors.ActorFunding.getValue(burnActorId, { at: block.hash }),
  ]);
  if (!assetDetails)
    throw new Error(`foreign asset ${foreignId} is unavailable`);
  if (!pool || !reserves.success)
    throw new Error('Native/foreign pool state is unavailable');
  if (!observation) throw new Error('Oracle observation is unavailable');
  if (!actorHot || !actorContractHead || !actorFunding)
    throw new Error('Burn Actor state is incomplete');

  return jsonValue({
    finalized_block: {
      number: block.number,
      hash: block.hash,
    },
    runtime: {
      spec_name: version.spec_name,
      impl_name: version.impl_name,
      authoring_version: version.authoring_version,
      spec_version: version.spec_version,
      impl_version: version.impl_version,
      system_version: version.system_version,
      transaction_version: version.transaction_version,
      code_hash: blake2AsHex(codeHex, 256),
    },
    preserved_state: {
      foreign_asset_id: foreignId,
      burn_actor_id: burnActorId,
      burn_actor_account: burnAccount,
      burn_native_balance: burnNative.data.free,
      burn_foreign_balance: burnForeign ?? 0n,
      foreign_asset_details: assetDetails,
      pool,
      reserves: reserves.value,
      router_lp_pairs: lpPairs,
      oracle_feed: feed,
      oracle_observation: observation,
      actor_identity: actorIdentity,
      actor_hot: actorHot,
      actor_contract_head: actorContractHead,
      actor_funding: actorFunding,
    },
  });
}

if (!['snapshot', 'verify'].includes(mode))
  throw new Error(`unsupported upgrade-state evidence mode: ${mode}`);
if (!statePath) throw new Error('UPGRADE_STATE_PATH is required');

const client = createWsClient(endpoint);
try {
  const api = client.getTypedApi(deos);
  const live = await chainSnapshot(client, api);
  if (mode === 'snapshot') {
    if (live.runtime.spec_version !== sourceSpecVersion)
      throw new Error(
        `source spec version mismatch: expected ${sourceSpecVersion}, found ${live.runtime.spec_version}`,
      );
    const payload = {
      schema_version: 1,
      endpoint,
      source: live,
    };
    await writeFile(statePath, `${JSON.stringify(payload, null, 2)}\n`, {
      flag: 'wx',
    });
    console.log(
      JSON.stringify({
        phase: 'baseline-state-captured',
        state_path: statePath,
        finalized_block: live.finalized_block.number,
        runtime_code_hash: live.runtime.code_hash,
      }),
    );
  } else {
    const baseline = JSON.parse(await readFile(statePath, 'utf8'));
    if (baseline.schema_version !== 1 || !baseline.source)
      throw new Error('baseline state evidence has an unsupported schema');
    if (baseline.source.runtime.spec_version !== sourceSpecVersion)
      throw new Error('baseline evidence does not identify the source runtime');
    if (live.runtime.spec_version !== targetSpecVersion)
      throw new Error(
        `target spec version mismatch: expected ${targetSpecVersion}, found ${live.runtime.spec_version}`,
      );
    if (live.runtime.spec_name !== baseline.source.runtime.spec_name)
      throw new Error('runtime spec_name changed across the upgrade');
    if (
      live.runtime.transaction_version !==
      baseline.source.runtime.transaction_version
    )
      throw new Error('transaction_version changed across the upgrade');
    if (!same(live.preserved_state, baseline.source.preserved_state))
      throw new Error(
        'preserved Router/Oracle/Actors state changed across upgrade',
      );

    const candidateBytes = await readFile(wasmPath);
    const candidateCodeHash = blake2AsHex(candidateBytes, 256);
    if (
      live.runtime.code_hash.toLowerCase() !== candidateCodeHash.toLowerCase()
    )
      throw new Error('live runtime code hash differs from candidate Wasm');

    console.log(
      JSON.stringify({
        phase: 'post-upgrade-state-preserved',
        source_spec_version: sourceSpecVersion,
        target_spec_version: targetSpecVersion,
        finalized_block: live.finalized_block.number,
        runtime_code_hash: live.runtime.code_hash,
        preserved_state: true,
      }),
    );
  }
} finally {
  client.destroy();
}
