/*
Domain: AAA deterministic feedback-analysis validation
Owns: Structural self/cross-actor loops, observation provenance, signal/shared-asset/actuator paths, bounds, and epistemic limits.
Excludes: Runtime execution, economic stability, probability, causal strength, scoring, and consensus behavior.
Zone: Web-client validation entrypoint; consumes the automation feedback-analysis contract only.
*/
import assert from 'node:assert/strict';
import test from 'node:test';

import { analyzeAaaFeedback } from '../src/lib/automation/feedback-analysis.ts';

const native = { type: 'Native', value: { $none: true } };
const bldr = {
  type: 'Local',
  value: { $runtimeType: 'number', $integer: '2' },
};
const priceFeed = {
  asset_in: native,
  asset_out: bldr,
  method: { type: 'PreExecutionSpot', value: { $none: true } },
  aggregation: {
    type: 'Ema',
    value: { half_life_blocks: { $runtimeType: 'number', $integer: '100' } },
  },
  scale: { $runtimeType: 'number', $integer: '12' },
};
const treasuryFeed = {
  asset_in: bldr,
  asset_out: native,
  method: { type: 'TreasuryRatio', value: { $none: true } },
  aggregation: { type: 'LastValue', value: { $none: true } },
  scale: { $runtimeType: 'number', $integer: '9' },
};

function projectionStep({
  index = 0,
  task = 'Transfer',
  conditions = [],
  reads = [],
  writes = [],
  recipients = [],
} = {}) {
  return {
    index,
    task,
    conditions,
    economicSurface: {
      assetsRead: reads,
      assetsWritten: writes,
      adapterDerivedAssetsRead: false,
      adapterDerivedAssetsWritten: false,
      recipients,
      transferExposure: task === 'Transfer' || task === 'SplitTransfer',
      mintExposure: task === 'Mint',
      burnExposure: task === 'Burn',
      liquidityMutation: [
        'AddLiquidity',
        'RemoveLiquidity',
        'DonateLiquidity',
      ].includes(task),
      stakingMutation: task === 'Stake' || task === 'Unstake',
      possibleActorSignals: recipients,
      committedNonCompensatedEffects: task !== 'StopCycle',
    },
  };
}

function observationCondition(
  feed,
  { type = 'ObservationBelow', maxAgeBlocks = 10 } = {},
) {
  return {
    type,
    observation: 'scalar-observation',
    readSurface: { feed, maxAgeBlocks },
  };
}

function analysis({ triggerFeeds = [], steps = [], actorType = 'User' } = {}) {
  return {
    provenance: 'StaticStructuralProjection',
    actorType,
    trigger: {
      observationFeeds: triggerFeeds,
    },
    steps,
    economicSurface: {
      assetsRead: [
        ...new Map(
          steps
            .flatMap((step) => step.economicSurface.assetsRead)
            .map((asset) => [JSON.stringify(asset), asset]),
        ).values(),
      ],
      assetsWritten: [
        ...new Map(
          steps
            .flatMap((step) => step.economicSurface.assetsWritten)
            .map((asset) => [JSON.stringify(asset), asset]),
        ).values(),
      ],
    },
  };
}

function epistemicLimits(component) {
  assert.equal(component.interpretation, 'StructuralPossibility');
  assert.equal(component.stability, 'Unknown');
  assert.equal(component.probability, 'Unknown');
  assert.equal(component.causalStrength, 'Unknown');
}

test('price to swap to price is a structural endogenous self-feedback path', () => {
  const model = analyzeAaaFeedback({
    actors: [
      {
        id: 'liquidity',
        analysis: analysis({
          triggerFeeds: [priceFeed],
          steps: [
            projectionStep({ task: 'SwapIn', reads: [native], writes: [bldr] }),
          ],
        }),
      },
    ],
    observations: [
      {
        id: 'ntve-bldr-price',
        feed: priceFeed,
        provenance: 'Endogenous',
        effectMatchers: [{ actorId: 'liquidity', effectClasses: ['Swap'] }],
      },
    ],
  });
  assert.equal(model.components.length, 1);
  assert.equal(model.components[0].kind, 'ReactiveSelfCycle');
  assert.deepEqual(model.components[0].actorIds, ['liquidity']);
  assert.deepEqual(model.components[0].observationIds, ['ntve-bldr-price']);
  assert.deepEqual(model.components[0].observationProvenance, ['Endogenous']);
  assert.deepEqual(model.components[0].canonicalPath, [
    'actor:liquidity',
    'observation:ntve-bldr-price',
    'actor:liquidity',
  ]);
  epistemicLimits(model.components[0]);
  assert(
    model.findings.some((finding) => finding.kind === 'ReactiveSelfCycle'),
  );
  assert(
    model.findings.some(
      (finding) => finding.kind === 'EndogenousObservationFeedback',
    ),
  );
});

test('reactive findings bind timing and policy claims to identified evidence', () => {
  const model = analyzeAaaFeedback({
    actors: [
      {
        id: 'system-market',
        analysis: analysis({
          actorType: 'System',
          triggerFeeds: [priceFeed],
          steps: [
            projectionStep({
              task: 'SwapIn',
              conditions: [
                observationCondition(priceFeed, { maxAgeBlocks: 3 }),
              ],
              reads: [native],
              writes: [bldr],
            }),
          ],
        }),
      },
      {
        id: 'other-market',
        analysis: analysis({
          steps: [
            projectionStep({
              task: 'SwapOut',
              reads: [bldr],
              writes: [native],
            }),
          ],
        }),
      },
    ],
    observations: [
      {
        id: 'price',
        feed: priceFeed,
        provenance: 'Endogenous',
        effectMatchers: [{ effectClasses: ['Swap'] }],
      },
    ],
    evidence: {
      identity: 'runtime-weights-cadence:fixture-1',
      runtimeIdentity: 'runtime:fixture-1',
      weightIdentity: 'weights:fixture-1',
      cadenceIdentity: 'cadence:fixture-1',
      estimatedDeliveryBlocks: 12,
      observationCadences: [
        { observationId: 'price', minimumUpdateIntervalBlocks: 1 },
      ],
      actorPolicies: [
        {
          actorId: 'system-market',
          cooldownBlocks: 5,
          hysteresis: 'Absent',
          persistenceBlocks: 0,
          gain: 'High',
          gainEvidenceIdentity: 'gain:declared-fixture-1',
          reactiveIngressPriority: 'Ordinary',
        },
      ],
    },
  });
  const kinds = new Set(model.findings.map((finding) => finding.kind));
  for (const kind of [
    'FreshnessWindowBelowEstimatedDeliveryEnvelope',
    'EndogenousObservationFeedback',
    'ReactiveCrossActorCycle',
    'ThresholdChatterRisk',
    'MissingHysteresisOrPersistence',
    'HighGainActuation',
    'CooldownFeedRateMismatch',
    'SharedObservationActuatorContention',
    'SystemActorWithoutReactiveIngressPriority',
  ]) {
    assert(kinds.has(kind), `missing ${kind}`);
  }
  assert.equal(model.evidenceIdentity, 'runtime-weights-cadence:fixture-1');
  assert.equal(model.evidenceSnapshot.runtimeIdentity, 'runtime:fixture-1');
  assert.equal(model.evidenceSnapshot.weightIdentity, 'weights:fixture-1');
  assert.equal(model.evidenceSnapshot.cadenceIdentity, 'cadence:fixture-1');
  for (const finding of model.findings.filter(
    (candidate) => 'evidenceIdentity' in candidate,
  )) {
    assert.equal(finding.evidenceIdentity, 'runtime-weights-cadence:fixture-1');
  }
});

test('fee funding to downstream market action to price forms a cross-actor path', () => {
  const marketAccount = {
    $runtimeType: 'AccountId32',
    $hex: `0x${'22'.repeat(32)}`,
  };
  const model = analyzeAaaFeedback({
    actors: [
      {
        id: 'fee-sink',
        sovereignAccount: {
          $runtimeType: 'AccountId32',
          $hex: `0x${'11'.repeat(32)}`,
        },
        analysis: analysis({
          triggerFeeds: [priceFeed],
          steps: [
            projectionStep({
              task: 'SplitTransfer',
              reads: [native],
              writes: [native],
              recipients: [{ kind: 'Explicit', value: marketAccount }],
            }),
          ],
        }),
      },
      {
        id: 'market-actor',
        sovereignAccount: marketAccount,
        analysis: analysis({
          steps: [
            projectionStep({ task: 'SwapIn', reads: [native], writes: [bldr] }),
          ],
        }),
      },
    ],
    observations: [
      {
        id: 'ntve-bldr-price',
        feed: priceFeed,
        provenance: 'Endogenous',
        effectMatchers: [{ actorId: 'market-actor', effectClasses: ['Swap'] }],
      },
    ],
  });
  const component = model.components.find(
    (candidate) => candidate.kind === 'ReactiveCrossActorCycle',
  );
  assert(component);
  assert.deepEqual(component.actorIds, ['fee-sink', 'market-actor']);
  assert(
    model.edges.some(
      (edge) =>
        edge.kind === 'ActorSignal' &&
        edge.from === 'actor:fee-sink' &&
        edge.to === 'actor:market-actor',
    ),
  );
  epistemicLimits(component);
  assert(
    model.findings.some(
      (finding) => finding.kind === 'ReactiveCrossActorCycle',
    ),
  );
});

test('shared assets expose actor funding and downstream activation paths', () => {
  const upstreamAccount = {
    $runtimeType: 'AccountId32',
    $hex: `0x${'33'.repeat(32)}`,
  };
  const downstreamAccount = {
    $runtimeType: 'AccountId32',
    $hex: `0x${'44'.repeat(32)}`,
  };
  const model = analyzeAaaFeedback({
    actors: [
      {
        id: 'upstream',
        sovereignAccount: upstreamAccount,
        analysis: analysis({
          steps: [
            projectionStep({
              task: 'Transfer',
              reads: [native],
              writes: [native],
              recipients: [{ kind: 'Explicit', value: downstreamAccount }],
            }),
          ],
        }),
      },
      {
        id: 'downstream',
        sovereignAccount: downstreamAccount,
        analysis: analysis({
          steps: [
            projectionStep({
              task: 'Transfer',
              reads: [native],
              writes: [native],
              recipients: [{ kind: 'Explicit', value: upstreamAccount }],
            }),
          ],
        }),
      },
    ],
    observations: [],
  });
  const component = model.components.find(
    (candidate) => candidate.kind === 'ReactiveCrossActorCycle',
  );
  assert(component);
  assert.deepEqual(component.actorIds, ['downstream', 'upstream']);
  assert.equal(component.assetNodeIds.length, 1);
  assert(model.edges.filter((edge) => edge.kind === 'ActorSignal').length >= 2);
  epistemicLimits(component);
});

test('typed parameter actuators remain explicit structural nodes', () => {
  const model = analyzeAaaFeedback({
    actors: [
      {
        id: 'treasury-policy',
        analysis: analysis({
          triggerFeeds: [priceFeed],
          steps: [projectionStep({ task: 'Transfer', reads: [native] })],
        }),
      },
    ],
    observations: [
      {
        id: 'market-price',
        feed: priceFeed,
        provenance: 'Unknown',
        effectMatchers: [],
      },
    ],
    parameterActuators: [
      {
        id: 'fee-rate',
        controlledByActorId: 'treasury-policy',
        affectsObservationIds: ['market-price'],
        affectsAssets: [],
      },
    ],
  });
  assert.equal(model.components.length, 1);
  assert.deepEqual(model.components[0].actuatorIds, ['fee-rate']);
  assert.deepEqual(model.components[0].observationProvenance, ['Unknown']);
  assert(model.edges.some((edge) => edge.kind === 'ParameterControl'));
  assert(
    model.edges.some((edge) => edge.kind === 'ParameterEffectOnObservation'),
  );
  epistemicLimits(model.components[0]);
});

test('exogenous and unmatched observations do not synthesize feedback', () => {
  const model = analyzeAaaFeedback({
    actors: [
      {
        id: 'release',
        analysis: analysis({
          triggerFeeds: [treasuryFeed],
          steps: [projectionStep({ task: 'Transfer', reads: [native] })],
        }),
      },
    ],
    observations: [
      {
        id: 'external-release-signal',
        feed: treasuryFeed,
        provenance: 'Exogenous',
        effectMatchers: [],
      },
    ],
  });
  assert.deepEqual(model.components, []);
  assert.equal(
    model.nodes.find((node) => node.kind === 'Observation').provenance,
    'Exogenous',
  );
});

test('feedback projection is deterministic and fails closed at graph bounds', () => {
  const input = {
    actors: [
      {
        id: 'actor',
        analysis: analysis({
          triggerFeeds: [priceFeed],
          steps: [projectionStep({ task: 'SwapIn', writes: [bldr] })],
        }),
      },
    ],
    observations: [
      {
        id: 'price',
        feed: priceFeed,
        provenance: 'Endogenous',
        effectMatchers: [{ effectClasses: ['Swap'] }],
      },
    ],
  };
  assert.deepEqual(analyzeAaaFeedback(input), analyzeAaaFeedback(input));
  assert.throws(
    () => analyzeAaaFeedback({ ...input, limits: { maxNodes: 1 } }),
    /node limit exceeded/,
  );
  assert.throws(
    () => analyzeAaaFeedback({ ...input, limits: { maxEdges: 1 } }),
    /edge limit exceeded/,
  );
  assert.throws(
    () =>
      analyzeAaaFeedback({
        ...input,
        parameterActuators: [
          {
            id: 'unknown',
            controlledByActorId: 'missing',
            affectsObservationIds: [],
            affectsAssets: [],
          },
        ],
      }),
    /Unknown actuator controller/,
  );
  assert.throws(
    () =>
      analyzeAaaFeedback({
        ...input,
        observations: [
          {
            ...input.observations[0],
            provenance: 'Exogenous',
          },
        ],
      }),
    /Exogenous observations cannot declare actor effects/,
  );
  assert.throws(
    () =>
      analyzeAaaFeedback({
        ...input,
        observations: [
          input.observations[0],
          { ...input.observations[0], id: 'duplicate-feed' },
        ],
      }),
    /Observation feed projections must be unique/,
  );
});
