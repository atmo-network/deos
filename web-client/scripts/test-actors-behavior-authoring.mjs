/*
Domain: Actors outcome-policy authoring and native metadata conformance
Owns: Explicit policy/funding fields, canonical IR v2, exact encoding and host limits.
Excludes: Runtime policy admission, financial service, production bindings and prices.
Set DEOS_ACTORS_TEST_METADATA_PATH for a temporary native schema export.
The default committed metadata must also satisfy these witnesses after binding.
*/
import { decAnyMetadata, metadata } from '@polkadot-api/substrate-bindings';
import assert from 'node:assert/strict';
import test from 'node:test';

import { readActorsTestMetadata } from './actors-test-metadata.mjs';

import { analyzeActorContract } from '../src/lib/automation/analysis.ts';
import {
  DEOS_ACTORS_AUTHORING_LIMITS,
  createActorArtifactFromAuthoring,
  createActorAuthoringContract,
  lowerActorAuthoringContract,
  validateActorAuthoringContract,
} from '../src/lib/automation/authoring.ts';
import { ACTOR_BEHAVIOR_METRICS } from '../src/lib/automation/behavior-policy.ts';
import {
  authoringContractToConfigurationIr,
  configurationIrToAuthoringContract,
  diagnoseActorConfigurationIr,
  diffActorConfigurationIr,
  lowerActorConfigurationIr,
  normalizeActorConfigurationIr,
  parseActorConfigurationJson,
  parseActorConfigurationMarkdown,
  parseActorConfigurationToml,
  serializeActorConfigurationJson,
  serializeActorConfigurationMarkdown,
  serializeActorConfigurationToml,
} from '../src/lib/automation/configuration-ir.ts';
import {
  createActorContractArtifact,
  encodeActorContractValue,
  inspectActorContractArtifact,
  readActorBehaviorContractSchema,
} from '../src/lib/automation/contract-artifact.ts';
import { ACTOR_CLOSE_REASONS } from '../src/lib/automation/eligibility.ts';
import {
  decodeActorRuntimeSimulationResult,
  encodeActorRuntimeSimulationResult,
} from '../src/lib/automation/runtime-simulation-codec.ts';
import { projectActorOutcomeHeader } from '../src/lib/automation/types.ts';

const metadataBytes = await readActorsTestMetadata();
const runtime = {
  genesisHash: `0x${'11'.repeat(32)}`,
  specVersion: 1,
  transactionVersion: 1,
};
const atom = (metric, threshold = 1) => ({ metric, threshold });
const policy = (...clauses) => ({ clauses });
const contract = (overrides = {}) => ({
  ...createActorAuthoringContract(),
  steps: [],
  ...overrides,
});
const limits = () => ({
  ...DEOS_ACTORS_AUTHORING_LIMITS,
  behavior: readActorBehaviorContractSchema(metadataBytes),
});
const artifact = (draft, bytes = metadataBytes, extra = {}) =>
  createActorArtifactFromAuthoring({
    contract: draft,
    metadataBytes: bytes,
    runtime,
    ...extra,
  });
const mutateMetadata = (mutate) => {
  const raw = decAnyMetadata(metadataBytes);
  mutate(raw.metadata.value);
  return metadata.enc(raw);
};
const entry = (value, suffix) =>
  value.lookup.find((item) => item.path.join('::').endsWith(suffix));

const weightModel = {
  identity: 'behavior-structural-fixture',
  version: '1',
  evaluationWeight: () => ({ refTime: 1n, proofSize: 1n }),
  evaluationFeeUpper: () => 0n,
  taskUpper: () => ({
    weight: { refTime: 1n, proofSize: 1n },
    executionFeeUpper: 1n,
  }),
  lifecycleOverhead: { weight: { refTime: 1n, proofSize: 1n }, fee: 1n },
  fundingPromotionOverhead: { weight: { refTime: 1n, proofSize: 1n }, fee: 1n },
};
const analyze = (value) =>
  analyzeActorContract({
    artifact: value,
    metadataBytes,
    runtime: { ...runtime, modelIdentity: 'native-schema-fixture' },
    weightModel,
  });
const simulationValue = (
  outcome,
  status = { type: 'Suspended', value: undefined },
) => ({
  success: true,
  value: {
    status,
    cycle_nonce: 1n,
    start_cursor: 0,
    run_cursor: 0,
    unsuccessful_attempts_at_cursor: 1,
    cumulative_outcomes: {
      executed_steps: 0,
      committed_effectful_tasks: 0,
      precondition_skips: 0,
      skipped_resolution: 0,
      skipped_funding_unavailable: 0,
      failed_steps: 0,
    },
    steps: [{ step_index: 0, outcome }],
  },
});

test('draft defaults declare absent outcome closure and fee-shortfall Close', () => {
  const draft = contract();
  assert.equal(draft.closeWhen, null);
  assert.equal(draft.onUnfunded, 'Close');
  assert.equal('completionPolicy' in draft, false);
  assert.equal(validateActorAuthoringContract(draft).valid, true);
});

test('unbound outcome limits and legacy completion are explicit refusals', () => {
  for (const draft of [
    contract({ closeWhen: policy([atom('ActionSuccessTotal')]) }),
    contract({ closeWhen: undefined }),
    contract({ completionPolicy: 'CloseAfterProductiveCycle' }),
    contract({ onUnfunded: 'Unknown' }),
  ]) {
    assert.equal(validateActorAuthoringContract(draft).valid, false);
    assert.throws(() => lowerActorAuthoringContract(draft));
  }
});

test('all ten metric variants and u32 extrema round-trip through the native Contract codec', () => {
  for (const metric of ACTOR_BEHAVIOR_METRICS) {
    for (const threshold of [1, 0xffff_ffff]) {
      const draft = contract({ closeWhen: policy([atom(metric, threshold)]) });
      const encoded = artifact(draft);
      const decoded = inspectActorContractArtifact(
        encoded,
        metadataBytes,
        runtime,
      );
      assert.equal(decoded.valid, true);
      assert.deepEqual(
        decoded.runtimeValue,
        lowerActorAuthoringContract(draft, limits()),
      );
      assert.deepEqual(decoded.runtimeValue.close_when, [
        [
          {
            metric: { type: metric, value: undefined },
            threshold,
          },
        ],
      ]);
      assert.equal('completion' in decoded.runtimeValue, false);
    }
  }
});

test('absent policy and mixed AND/OR policy preserve exact optional sequence encoding', () => {
  const absent = inspectActorContractArtifact(
    artifact(contract()),
    metadataBytes,
    runtime,
  );
  assert.equal(absent.valid, true);
  assert.equal(absent.runtimeValue.close_when, undefined);
  const draft = contract({
    closeWhen: policy(
      [atom('ActionSuccessTotal', 3), atom('CycleCompletedTotal', 2)],
      [atom('ActionFailureTotal', 5)],
    ),
  });
  const decoded = inspectActorContractArtifact(
    artifact(draft),
    metadataBytes,
    runtime,
  );
  assert.equal(decoded.valid, true);
  assert.deepEqual(
    decoded.runtimeValue,
    lowerActorAuthoringContract(draft, limits()),
  );
  assert.notEqual(artifact(draft).contractId, absent.artifact.contractId);
});

test('AwaitFunding is refused for both classes until public admission opens', () => {
  for (const mutability of ['Mutable', 'Immutable']) {
    // The runtime admits only Close while public financial-wait admission stays disabled, so a
    // User contract carrying AwaitFunding could never be submitted either.
    assert.throws(
      () => artifact(contract({ mutability, onUnfunded: 'AwaitFunding' })),
      /public runtime admission/,
    );
    const system = contract({
      actorType: 'System',
      mutability,
      fundingPolicy: { type: 'RuntimePolicy' },
    });
    assert.equal(
      inspectActorContractArtifact(artifact(system), metadataBytes, runtime)
        .valid,
      true,
    );
    assert.throws(
      () => artifact({ ...system, onUnfunded: 'AwaitFunding' }),
      /public runtime admission/,
    );
  }
});

test('metadata field and metric vocabulary drift refuses before authoring encoding', () => {
  const legacy = mutateMetadata((value) => {
    entry(value, 'types::contract::ActorContract').def.value[5].name =
      'completion';
  });
  assert.throws(
    () => artifact(contract(), legacy),
    /does not support close_when/,
  );
  const changedMetric = mutateMetadata((value) => {
    entry(value, 'types::behavior::BehaviorMetric').def.value[0].index = 10;
  });
  assert.throws(() => artifact(contract(), changedMetric), /metric catalog/);
  const changedFunding = mutateMetadata((value) => {
    entry(value, 'types::lifecycle::OnUnfunded').def.value[1].name =
      'WaitForever';
  });
  assert.throws(
    () => artifact(contract(), changedFunding),
    /on_unfunded vocabulary/,
  );
});

test('caller limits cannot override metadata clause or atom bounds', () => {
  const constrained = mutateMetadata((value) => {
    const owner = value.pallets.find((pallet) =>
      pallet.constants.some((item) => item.name === 'MaxBehaviorClauses'),
    );
    for (const name of ['MaxBehaviorClauses', 'MaxBehaviorAtomsPerClause']) {
      owner.constants.find((item) => item.name === name).value = '0x01000000';
    }
  });
  const spoofed = {
    ...DEOS_ACTORS_AUTHORING_LIMITS,
    behavior: { maxClauses: 100, maxAtomsPerClause: 100 },
  };
  for (const closeWhen of [
    policy([atom('ActionSuccessTotal')], [atom('ActionFailureTotal')]),
    policy([atom('ActionSuccessTotal'), atom('CycleCompletedTotal')]),
  ]) {
    assert.throws(
      () => artifact(contract({ closeWhen }), constrained, { limits: spoofed }),
      /1\.\.1/,
    );
  }
});

test('IR v2 syntax adapters preserve explicit outcomes, funding and exact artifact identity', () => {
  const draft = contract({
    onUnfunded: 'Close',
    closeWhen: policy([
      atom('ActionSuccessStreak', 3),
      atom('CycleCompletedTotal', 2),
    ]),
  });
  const ir = authoringContractToConfigurationIr(draft);
  assert.equal(ir.formatVersion, 2);
  assert.equal('completionPolicy' in ir, false);
  const expected = artifact(draft);
  for (const [serialize, parse] of [
    [serializeActorConfigurationJson, parseActorConfigurationJson],
    [serializeActorConfigurationToml, parseActorConfigurationToml],
    [serializeActorConfigurationMarkdown, parseActorConfigurationMarkdown],
  ]) {
    const restored = parse(serialize(ir));
    assert.deepEqual(restored, ir);
    assert.deepEqual(diagnoseActorConfigurationIr(restored, limits()), []);
    assert.deepEqual(
      lowerActorConfigurationIr(restored, limits()),
      lowerActorAuthoringContract(draft, limits()),
    );
    assert.deepEqual(
      artifact(configurationIrToAuthoringContract(restored)),
      expected,
    );
  }
});

test('IR v1 and legacy fields refuse; structural diff keeps outcome and funding changes distinct', () => {
  const before = authoringContractToConfigurationIr(contract());
  assert.throws(
    () => normalizeActorConfigurationIr({ ...before, formatVersion: 1 }),
    /version/,
  );
  assert.throws(
    () =>
      normalizeActorConfigurationIr({
        ...before,
        completionPolicy: 'Persistent',
      }),
    /Unknown/,
  );
  const after = authoringContractToConfigurationIr(
    contract({
      closeWhen: policy([atom('CycleCleanTotal', 2)]),
      onUnfunded: 'Close',
    }),
  );
  // Only Close is authorable while public financial-wait admission stays disabled, so this
  // fixture proves close-policy preservation rather than a funding-policy change.
  assert.deepEqual(
    diffActorConfigurationIr(before, after).map((change) => change.path),
    ['/closeWhen'],
  );
});

test('static analysis preserves exact authored policy and labels unqualified new resource owners', () => {
  const draft = contract({
    closeWhen: policy([atom('CycleCompletedTotal', 3)]),
    onUnfunded: 'Close',
  });
  const result = analyze(artifact(draft));
  assert.equal(result.identity.analyzerVersion, '13');
  assert.deepEqual(result.closeWhen, draft.closeWhen);
  assert.equal(result.onUnfunded, 'Close');
  // FinancialWait is excluded only while the contract carries AwaitFunding, so a Close contract
  // labels OutcomePolicy alone; that difference is the analyzer's own signal, not a regression.
  assert.deepEqual(result.costCoverage, {
    scope: 'ExistingStructuralModel',
    excludedOwners: ['OutcomePolicy'],
  });
  assert.equal('completionPolicy' in result, false);
  assert.deepEqual(
    analyze(artifact(contract())).costCoverage.excludedOwners,
    [],
  );
});

test('static analysis rejects noncanonical policy even when its SCALE shape is valid', () => {
  const value = lowerActorAuthoringContract(contract());
  value.close_when = [
    [
      {
        metric: { type: 'ActionSuccessTotal', value: undefined },
        threshold: 0,
      },
    ],
  ];
  const encoded = createActorContractArtifact({
    metadataBytes,
    runtime,
    actorType: 'User',
    mutability: 'Mutable',
    contractScale: encodeActorContractValue(metadataBytes, value),
  });
  assert.throws(() => analyze(encoded), /positive u32/);
});

test('live header projection exposes presence and funding choice without inventing an expression', () => {
  for (const has_close_when of [false, true]) {
    assert.deepEqual(
      projectActorOutcomeHeader({
        has_close_when,
        on_unfunded: { type: 'Close', value: undefined },
      }),
      { hasCloseWhen: has_close_when, onUnfunded: 'Close' },
    );
  }
  for (const header of [
    null,
    { completion: { type: 'Persistent' } },
    { has_close_when: 1, on_unfunded: { type: 'Close' } },
    { has_close_when: false, on_unfunded: { type: 'WaitForever' } },
    { has_close_when: false, on_unfunded: { type: 'Close', value: 1 } },
  ])
    assert.throws(() => projectActorOutcomeHeader(header));
});

test('native simulation decoding distinguishes service fees from Task inputs, including skipped receipts', () => {
  for (const reason of ['ServiceFee', 'TaskInput']) {
    for (const skipped of [false, true]) {
      const unavailable = {
        type: 'FundingUnavailable',
        value: { type: reason, value: undefined },
      };
      const decoded = decodeActorRuntimeSimulationResult(
        metadataBytes,
        encodeActorRuntimeSimulationResult(
          metadataBytes,
          simulationValue(
            skipped ? { type: 'Skipped', value: unavailable } : unavailable,
          ),
        ),
      );
      assert.equal(decoded.success, true);
      assert.deepEqual(
        decoded.outcome.steps[0].outcome,
        skipped
          ? {
              type: 'Skipped',
              reason: 'FundingUnavailable',
              fundingReason: reason,
            }
          : { type: 'FundingUnavailable', reason },
      );
    }
  }
});

test('Close vocabulary matches native metadata and rejects the removed productive reason', () => {
  const raw = decAnyMetadata(metadataBytes).metadata.value;
  assert.deepEqual(
    entry(raw, 'types::lifecycle::CloseReason').def.value.map(
      (variant) => variant.name,
    ),
    ACTOR_CLOSE_REASONS,
  );
  for (const reason of ACTOR_CLOSE_REASONS) {
    const value = simulationValue(
      { type: 'Stopped', value: undefined },
      { type: 'Closed', value: { type: reason, value: undefined } },
    );
    const decoded = decodeActorRuntimeSimulationResult(
      metadataBytes,
      encodeActorRuntimeSimulationResult(metadataBytes, value),
    );
    assert.equal(decoded.success, true);
    assert.equal(decoded.outcome.closeReason, reason);
  }
  const legacy = mutateMetadata((value) => {
    entry(value, 'types::lifecycle::CloseReason').def.value.find(
      (variant) => variant.name === 'BehaviorPolicySatisfied',
    ).name = 'ProductiveCycleCompleted';
  });
  const value = simulationValue(
    { type: 'Stopped', value: undefined },
    {
      type: 'Closed',
      value: { type: 'ProductiveCycleCompleted', value: undefined },
    },
  );
  assert.throws(
    () =>
      decodeActorRuntimeSimulationResult(
        legacy,
        encodeActorRuntimeSimulationResult(legacy, value),
      ),
    /Unsupported runtime Close reason/,
  );
});
