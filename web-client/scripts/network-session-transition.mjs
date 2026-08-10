#!/usr/bin/env node

/*
Domain: Live session-transition evidence
Owns: Finalized Session index observation across both DEOS collator RPC views.
Excludes: Network spawning, collator process evidence, session policy, and release-profile orchestration.
Zone: Live read-only assurance atom invoked by the root network harness.
*/
import { deos } from '@polkadot-api/descriptors';
import { createWsClient } from 'polkadot-api/ws';

const primaryEndpoint =
  process.env.DEOS_PRIMARY_WS_ENDPOINT ?? 'ws://127.0.0.1:9988';
const secondaryEndpoint =
  process.env.DEOS_SECONDARY_WS_ENDPOINT ?? 'ws://127.0.0.1:9999';
const timeoutLiteral = process.env.DEOS_SESSION_TIMEOUT_MS ?? '28800000';
const pollLiteral = process.env.DEOS_SESSION_POLL_MS ?? '6000';
const stallLiteral = process.env.DEOS_SESSION_STALL_MS ?? '120000';

if (process.argv.includes('--help') || process.argv.includes('-h')) {
  console.log(`Usage: network-session-transition.mjs

Observe one finalized Session.CurrentIndex transition through both live collator
RPC views. Require continuing finalized progress and one equal non-empty validator
set after the transition. This command is read-only and may run for several hours.

Environment:
  DEOS_PRIMARY_WS_ENDPOINT=ws://127.0.0.1:9988
  DEOS_SECONDARY_WS_ENDPOINT=ws://127.0.0.1:9999
  DEOS_SESSION_TIMEOUT_MS=28800000
  DEOS_SESSION_POLL_MS=6000
  DEOS_SESSION_STALL_MS=120000`);
  process.exit(0);
}
if (process.argv.length > 2)
  throw new Error(`unknown argument: ${process.argv[2]}`);

function positiveSafeInteger(literal, label) {
  if (!/^[1-9][0-9]*$/.test(literal))
    throw new Error(`${label} must be a complete positive integer literal`);
  const value = Number(literal);
  if (!Number.isSafeInteger(value))
    throw new Error(`${label} exceeds the safe integer range`);
  return value;
}

const timeoutMs = positiveSafeInteger(timeoutLiteral, 'session timeout');
const pollMs = positiveSafeInteger(pollLiteral, 'session poll interval');
const stallMs = positiveSafeInteger(stallLiteral, 'session stall timeout');
if (stallMs >= timeoutMs)
  throw new Error('session stall timeout must be shorter than total timeout');

function sleep(duration) {
  return new Promise((resolve) => setTimeout(resolve, duration));
}

function sessionIndex(value) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 0)
    throw new Error(`Session.CurrentIndex is invalid: ${String(value)}`);
  return parsed;
}

function validatorIdentity(value) {
  if (typeof value === 'string') return value;
  return JSON.stringify(value);
}

async function snapshot(client, api) {
  const block = await client.getFinalizedBlock();
  const [index, validators] = await Promise.all([
    api.query.Session.CurrentIndex.getValue({ at: block.hash }),
    api.query.Session.Validators.getValue({ at: block.hash }),
  ]);
  return {
    blockHash: block.hash,
    blockNumber: block.number,
    index: sessionIndex(index),
    validators: validators.map(validatorIdentity),
  };
}

function assertMatchingValidators(primary, secondary) {
  if (primary.validators.length === 0)
    throw new Error('finalized Session.Validators is empty');
  if (
    JSON.stringify(primary.validators) !== JSON.stringify(secondary.validators)
  )
    throw new Error(
      'collator RPC views disagree on finalized Session.Validators',
    );
}

const primaryClient = createWsClient(primaryEndpoint);
const secondaryClient = createWsClient(secondaryEndpoint);
try {
  const primaryApi = primaryClient.getTypedApi(deos);
  const secondaryApi = secondaryClient.getTypedApi(deos);
  const [initialPrimary, initialSecondary] = await Promise.all([
    snapshot(primaryClient, primaryApi),
    snapshot(secondaryClient, secondaryApi),
  ]);
  if (initialPrimary.index !== initialSecondary.index)
    throw new Error(
      'collator RPC views disagree on the initial finalized session',
    );
  assertMatchingValidators(initialPrimary, initialSecondary);

  const startedAt = Date.now();
  let lastAdvanceAt = startedAt;
  let lastPrimaryBlock = initialPrimary.blockNumber;
  let finalPrimary;
  let finalSecondary;
  while (Date.now() - startedAt <= timeoutMs) {
    await sleep(pollMs);
    const primary = await snapshot(primaryClient, primaryApi);
    if (primary.blockNumber > lastPrimaryBlock) {
      lastPrimaryBlock = primary.blockNumber;
      lastAdvanceAt = Date.now();
    } else if (Date.now() - lastAdvanceAt > stallMs) {
      throw new Error(
        `finalized parachain progress stalled at block ${primary.blockNumber}`,
      );
    }
    if (primary.index <= initialPrimary.index) continue;

    const secondary = await snapshot(secondaryClient, secondaryApi);
    if (secondary.index < primary.index) continue;
    if (secondary.index !== primary.index)
      throw new Error(
        'collator RPC views skipped to different finalized sessions',
      );
    assertMatchingValidators(primary, secondary);
    finalPrimary = primary;
    finalSecondary = secondary;
    break;
  }

  if (!finalPrimary || !finalSecondary)
    throw new Error(
      `no finalized session transition observed within ${timeoutMs}ms`,
    );
  if (finalPrimary.blockNumber <= initialPrimary.blockNumber)
    throw new Error(
      'session index changed without finalized block progression',
    );

  console.log(
    JSON.stringify({
      primary_endpoint: primaryEndpoint,
      secondary_endpoint: secondaryEndpoint,
      initial_session: initialPrimary.index,
      finalized_session: finalPrimary.index,
      initial_primary_block: initialPrimary.blockNumber,
      finalized_primary_block: finalPrimary.blockNumber,
      finalized_secondary_block: finalSecondary.blockNumber,
      validator_count: finalPrimary.validators.length,
    }),
  );
} finally {
  primaryClient.destroy();
  secondaryClient.destroy();
}
