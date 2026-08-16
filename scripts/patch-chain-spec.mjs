#!/usr/bin/env node

import { readFile, writeFile } from 'node:fs/promises';
import process from 'node:process';

function fail(message) { throw new Error(message); }
function integer(value, label) { if (!/^(0|[1-9][0-9]*)$/.test(value)) fail(`${label} must be an unsigned integer`); return BigInt(value); }
async function main(args) {
  if (args.includes('--help') || args.length !== 9) { console.log('Usage: patch-chain-spec.mjs SPEC CHAIN_TYPE CHAIN_NAME CHAIN_ID NATIVE_ID FOREIGN_ID INITIAL_PRICE SLOPE FOREIGN_BALANCE'); return; }
  const [specPath, chainType, chainName, chainId, nativeText, foreignText, priceText, slopeText, balanceText] = args;
  const nativeId = Number(integer(nativeText, 'native ID'));
  const foreignId = Number(integer(foreignText, 'foreign ID'));
  const initialPrice = Number(integer(priceText, 'initial price'));
  const slope = Number(integer(slopeText, 'slope'));
  const foreignBalance = integer(balanceText, 'foreign balance');
  for (const [value, label] of [[nativeId,'native ID'],[foreignId,'foreign ID'],[initialPrice,'initial price'],[slope,'slope']]) if (!Number.isSafeInteger(value)) fail(`${label} exceeds the JSON safe integer range`);
  const spec = JSON.parse(await readFile(specPath, 'utf8'));
  spec.chainType = chainType; spec.name = chainName; spec.id = chainId;
  const patch = spec.genesis ??= {}; const runtime = patch.runtimeGenesis ??= {}; const values = runtime.patch ??= {};
  delete values.sudo;
  const assets = values.assets ??= {}; assets.nextAssetId ??= null; assets.reserves ??= [];
  const assetEntries = assets.assets ??= []; const metadata = assets.metadata ??= []; const accounts = assets.accounts ??= [];
  const owner = values.balances?.balances?.[0]?.[0];
  if (owner === undefined) fail('Chain spec requires an endowed bootstrap asset owner');
  const add = (array, item) => { if (!array.some((entry) => JSON.stringify(entry) === JSON.stringify(item))) array.push(item); };
  const balanceToken = '__DEOS_EXACT_FOREIGN_BALANCE__';
  const upsertAccount = (assetId) => {
    const replacement = [assetId, owner, balanceToken];
    const index = accounts.findIndex((entry) => Array.isArray(entry) && entry[0] === assetId && entry[1] === owner);
    if (index < 0) accounts.push(replacement);
    else { accounts[index] = replacement; for (let cursor = accounts.length - 1; cursor > index; cursor -= 1) if (Array.isArray(accounts[cursor]) && accounts[cursor][0] === assetId && accounts[cursor][1] === owner) accounts.splice(cursor, 1); }
  };
  add(assetEntries, [nativeId, owner, true, 1]); add(metadata, [nativeId, [...Buffer.from('Native Staking Token')], [...Buffer.from('NTVE')], 12]); upsertAccount(nativeId);
  add(assetEntries, [foreignId, owner, true, 1]); add(metadata, [foreignId, [...Buffer.from('Foreign Token')], [...Buffer.from('FRGN')], 12]); upsertAccount(foreignId);
  values.tokenMintingCurve = { curves: [['Native', { Foreign: foreignId }, initialPrice, slope]] };
  values.staking = { registeredAssets: [nativeId] };
  const encoded = JSON.stringify(spec, null, 2).replaceAll(`"${balanceToken}"`, foreignBalance.toString());
  await writeFile(specPath, `${encoded}\n`);
}
main(process.argv.slice(2)).catch((error) => { console.error(`patch-chain-spec: ${error.message}`); process.exitCode = 1; });
