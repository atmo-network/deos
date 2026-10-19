/*
Domain: Governance power SCALE contract
Owns: Actual metadata limb codecs and exact scalar projection at every power boundary.
Excludes: Metadata issuance, voting policy, prices, Wasm and release acceptance.
*/
import {
  getDynamicBuilder,
  getLookupFn,
} from '@polkadot-api/metadata-builders';
import {
  decAnyMetadata,
  unifyMetadata,
} from '@polkadot-api/substrate-bindings';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

import { readActorsTestMetadata } from './actors-test-metadata.mjs';

import { decodeGovernancePower } from '../src/lib/adapters/governance/power.ts';

const metadata = unifyMetadata(decAnyMetadata(await readActorsTestMetadata()));
const builder = getDynamicBuilder(getLookupFn(metadata));
const source = await readFile(
  new URL('../src/lib/adapters/governance/papi.ts', import.meta.url),
  'utf8',
);
const ast = ts.createSourceFile(
  'papi.ts',
  source,
  ts.ScriptTarget.Latest,
  true,
);
const names = [
  'mapProposalStatus',
  'mapProposalVoteTally',
  'mapAccountPowerView',
  'mapProposalPrimaryTrackTally',
  'mapFinalizedProposalOutcome',
  'mapProposalResolutionState',
  'mapVotePowerProfile',
  'mapProposalVoteKind',
  'mapPrimaryTrackOption',
  'mapRejectionReason',
  'mapVetoCancellationMode',
];
// Execute the actual pure adapter functions without initializing wallet/RPC imports.
const functions = ast.statements.filter(
  (node) => ts.isFunctionDeclaration(node) && names.includes(node.name?.text),
);
assert.equal(functions.length, names.length);
const javascript = ts.transpileModule(
  functions.map((node) => node.getText(ast)).join('\n'),
  {
    compilerOptions: {
      target: ts.ScriptTarget.ES2022,
      module: ts.ModuleKind.CommonJS,
    },
  },
).outputText;
const maps = new Function(
  'decodeGovernancePower',
  `${javascript}; return { ${names.join(',')} };`,
)(decodeGovernancePower);

function codec(name) {
  const entries = metadata.lookup.filter(
    (entry) =>
      entry.path[0] === 'pallet_deos_governance' && entry.path.at(-1) === name,
  );
  assert.equal(entries.length, 1, `One canonical ${name}`);
  return builder.buildDefinition(entries[0].id);
}

const limbs = [0x0123456789abcdefn, 0xfedcba9876543210n, 7n, 11n];
const scalar = limbs[0] + (limbs[1] << 64n) + (7n << 128n) + (11n << 192n);
const max = (1n << 256n) - 1n;
const full = Array(4).fill((1n << 64n) - 1n);
const roundtrip = (name, value) => {
  const c = codec(name);
  const encoded = c.enc(value);
  const decoded = c.dec(encoded);
  assert.deepEqual(decoded, value);
  assert.deepEqual(c.enc(decoded), encoded);
  return decoded;
};

test('U256 little-endian limbs preserve asymmetric, above-u128 and full-range values', () => {
  assert.equal(decodeGovernancePower(limbs), scalar);
  assert.equal(decodeGovernancePower([0n, 0n, 1n, 0n]), 1n << 128n);
  assert.equal(decodeGovernancePower(full), max);
  assert.equal(decodeGovernancePower([0n, 0n, 0n, 0n]), 0n);
  for (const value of [
    0n,
    null,
    {},
    [],
    [1n],
    [0n, 0n, 0n, 0n, 0n],
    [-1n, 0n, 0n, 0n],
    [1n << 64n, 0n, 0n, 0n],
    [0, 0n, 0n, 0n],
    Array(4),
  ]) {
    assert.throws(
      () => decodeGovernancePower(value),
      /Invalid Governance U256 limbs/,
    );
  }
});

test('316-byte tally roundtrips actual metadata and maps every wide field', () => {
  const tally = Object.fromEntries(
    ['aye', 'nay', 'amplify', 'approve', 'reduce', 'veto', 'pass'].map(
      (key) => [`${key}_voters`, 1],
    ),
  );
  const weights = [
    'aye',
    'nay',
    'amplify',
    'approve',
    'reduce',
    'veto',
    'pass',
    'turnout',
    'veto_turnout',
  ];
  for (const key of weights)
    tally[`${key}_weight`] = key === 'aye' ? full : limbs;
  const decoded = roundtrip('ProposalVoteTally', tally);
  const c = codec('ProposalVoteTally');
  const encoded = c.enc(tally);
  assert.equal(encoded.length, 316);
  assert.throws(() => c.dec(encoded.slice(0, -1)));
  const mapped = maps.mapProposalVoteTally(decoded);
  for (const key of weights) {
    const camel = key.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
    assert.equal(mapped[`${camel}Weight`], key === 'aye' ? max : scalar);
  }
});

test('binary and invoice primary tallies convert every weight', () => {
  for (const [type, keys] of [
    ['Binary', ['aye', 'nay', 'turnout']],
    [
      'Invoice',
      [
        'amplify',
        'approve',
        'reduce',
        'nay',
        'positive',
        'turnout',
        'leading_positive',
      ],
    ],
  ]) {
    const value =
      type === 'Binary'
        ? { leading_option: undefined }
        : { leading_positive_option: undefined };
    for (const key of keys) {
      value[`${key}_weight`] = limbs;
      if (!['turnout', 'positive', 'leading_positive'].includes(key))
        value[`${key}_voters`] = 1;
    }
    const mapped = maps.mapProposalPrimaryTrackTally(
      roundtrip('ProposalPrimaryTrackTally', { type, value }),
    );
    for (const key of keys)
      assert.equal(
        mapped[
          `${key.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase())}Weight`
        ],
        scalar,
      );
  }
});

test('account current/frozen power, veto resolution and final outcome convert exactly', () => {
  const ballot = {
    vote: { type: 'Aye', value: undefined },
    vote_epoch: 3,
    weight: limbs,
    raw_power: full,
  };
  const view = {
    governance_lock_until: undefined,
    ordinary_power_profile: { type: 'DecliningDirectStake', value: undefined },
    protection_power_profile: { type: 'DecliningVetoAsset', value: undefined },
    current_ordinary_weight: limbs,
    current_protection_weight: full,
    current_protection_raw_power: limbs,
    frozen_ordinary_ballot: ballot,
    frozen_protection_ballot: ballot,
  };
  const mapped = maps.mapAccountPowerView(
    roundtrip('AccountGovernancePowerView', view),
  );
  assert.equal(mapped.currentOrdinaryWeight, scalar);
  assert.equal(mapped.currentProtectionWeight, max);
  assert.equal(mapped.currentProtectionRawPower, scalar);
  for (const key of ['frozenOrdinaryBallot', 'frozenProtectionBallot']) {
    assert.equal(mapped[key].weight, scalar);
    assert.equal(mapped[key].rawPower, max);
  }
  const resolution = maps.mapProposalResolutionState(
    roundtrip('ProposalResolutionState', {
      type: 'VetoPassing',
      value: {
        veto_weight: limbs,
        pass_weight: full,
        mode: { type: 'TrackOutcome', value: undefined },
      },
    }),
  );
  assert.equal(resolution.vetoWeight, scalar);
  assert.equal(resolution.passWeight, max);
  const outcome = maps.mapFinalizedProposalOutcome(
    roundtrip('FinalizedProposalOutcome', {
      type: 'VetoCancelled',
      value: { finalized_epoch: 3, veto_weight: full },
    }),
  );
  assert.equal(outcome.vetoWeight, max);
  const status = maps.mapProposalStatus(
    roundtrip('ProposalStatus', {
      type: 'Active',
      value: {
        type: 'VetoPassing',
        value: {
          veto_weight: full,
          pass_weight: limbs,
          mode: { type: 'TrackOutcome', value: undefined },
        },
      },
    }),
  );
  assert.equal(status.resolution.vetoWeight, max);
  assert.equal(status.resolution.passWeight, scalar);
  const finalized = maps.mapProposalStatus(
    roundtrip('ProposalStatus', {
      type: 'Finalized',
      value: {
        type: 'VetoCancelled',
        value: {
          finalized_epoch: 3,
          veto_weight: limbs,
        },
      },
    }),
  );
  assert.equal(finalized.outcome.vetoWeight, scalar);
});
