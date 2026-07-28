/*
Domain: AAA deterministic feedback-analysis validation
Owns: Structural self/cross-actor loops, observation provenance, signal/shared-asset/actuator paths, bounds, and epistemic limits.
Excludes: Runtime execution, economic stability, probability, causal strength, scoring, and consensus behavior.
Zone: Web-client validation entrypoint; consumes the automation feedback-analysis contract only.
*/
import assert from 'node:assert/strict';
import test from 'node:test';

import { analyzeAaaFeedback } from '../src/lib/automation/feedback-analysis.ts';
import { DEOS_OBSERVATION_RUNTIME_EVIDENCE } from '../src/lib/observation/runtime-evidence.generated.ts';

const runtimeEvidence = DEOS_OBSERVATION_RUNTIME_EVIDENCE;
const verifiedRuntimeIdentity = `${runtimeEvidence.runtime.specName}@spec-${runtimeEvidence.runtime.specVersion} · code:${runtimeEvidence.runtimeCodeHash} · metadata:${runtimeEvidence.metadataHash}`;
const schedulerEvidence = {
  maxServiceUnitsPerBlock: runtimeEvidence.fanout.maxServiceUnitsPerBlock,
  maxActiveDirtyFeeds: runtimeEvidence.fanout.maxActiveDirtyFeeds,
  maxSubscriberPagesPerFeed: runtimeEvidence.fanout.maxSubscriberPagesPerFeed,
};

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

function analysis({
  triggerFeeds = [],
  steps = [],
  actorType = 'User',
  cooldownBlocks = 0,
} = {}) {
  return {
    provenance: 'StaticStructuralProjection',
    identity: {
      planId: 'artifact:test-plan',
      genesisHash: 'runtime:test-genesis',
      metadataHash: 'runtime:test-metadata',
      specVersion: 1,
      transactionVersion: 1,
      runtimeModelIdentity: 'runtime:test-model',
      weightModelIdentity: 'weights:test-model',
      adapterCapabilityIdentity: null,
      minimumBalanceEvidenceIdentity: null,
      minimumBalanceEvidenceBlockHash: null,
      analyzerVersion: '3',
    },
    actorType,
    cooldownBlocks,
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

function exactResource(kind, identity, access) {
  return {
    kind,
    identity,
    access,
    evidence: {
      provenance: 'RuntimeDerived',
      identity: 'state:finalized-fixture',
    },
  };
}

function sovereignAccount(value) {
  return {
    value,
    evidence: {
      provenance: 'RuntimeDerived',
      identity: 'state:finalized-fixture',
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
        producer: 'AxialRouterPreExecutionReserves',
        lifecycle: 'Active',
        evidence: {
          provenance: 'RuntimeDerived',
          identity: 'state:observation-fixture',
        },
        effectMatchers: [
          {
            actorId: 'liquidity',
            effectClasses: ['Swap'],
            evidenceIdentity: 'declaration:liquidity-price-effect',
          },
        ],
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
  assert.deepEqual(
    model.components[0].canonicalEdges.map((edge) => [
      edge.kind,
      edge.family,
      edge.provenance,
      edge.evidenceIdentities,
    ]),
    [
      [
        'ActorEffectOnObservation',
        'ReactiveCausal',
        'Declared',
        [
          'artifact:test-plan',
          'declaration:liquidity-price-effect',
          'state:observation-fixture',
        ],
      ],
      [
        'ObservationTrigger',
        'ReactiveCausal',
        'ArtifactDerived',
        ['artifact:test-plan', 'state:observation-fixture'],
      ],
    ],
  );
  epistemicLimits(model.components[0]);
  assert(
    model.nodes
      .filter((node) => node.kind === 'Resource')
      .every(
        (node) =>
          node.resourceKind === 'Unknown' && node.actorId === 'liquidity',
      ),
  );
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
          cooldownBlocks: 5,
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
        producer: 'AxialRouterPreExecutionReserves',
        lifecycle: 'Active',
        evidence: {
          provenance: 'RuntimeDerived',
          identity: 'state:observation-fixture',
        },
        effectMatchers: [
          {
            effectClasses: ['Swap'],
            evidenceIdentity: 'declaration:swap-price-effect',
          },
        ],
      },
    ],
    evidence: {
      identity: 'runtime-weights-cadence:fixture-1',
      runtimeIdentity: verifiedRuntimeIdentity,
      runtimeVerification: {
        status: 'Verified',
        observedIdentity: verifiedRuntimeIdentity,
        scheduler: schedulerEvidence,
        reasons: [],
      },
      weightIdentity: runtimeEvidence.weightIdentity,
      cadenceIdentity: 'cadence:fixture-1',
      estimatedDeliveryBlocks: 12,
      estimatedDeliveryEvidence: {
        provenance: 'RuntimeDerived',
        identity: 'runtime-weights-cadence:fixture-1',
      },
      observationCadences: [
        {
          observationId: 'price',
          minimumUpdateIntervalBlocks: 1,
          evidence: {
            provenance: 'RuntimeDerived',
            identity: 'cadence:fixture-1',
          },
        },
      ],
      actorPolicies: [
        {
          actorId: 'system-market',
          gain: 'High',
          gainEvidence: {
            provenance: 'Declared',
            identity: 'gain:declared-fixture-1',
          },
          reactiveIngressPriority: 'Ordinary',
          reactiveIngressPriorityEvidence: {
            provenance: 'RuntimeDerived',
            identity: verifiedRuntimeIdentity,
          },
        },
      ],
    },
  });
  const kinds = new Set(model.findings.map((finding) => finding.kind));
  for (const kind of [
    'FreshnessWindowBelowEstimatedDeliveryEnvelope',
    'EndogenousObservationFeedback',
    'ReactiveSelfCycle',
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
  assert.equal(model.evidenceSnapshot.runtimeIdentity, verifiedRuntimeIdentity);
  assert.equal(
    model.evidenceSnapshot.weightIdentity,
    runtimeEvidence.weightIdentity,
  );
  assert.equal(model.evidenceStatus, 'Verified');
  assert.equal(model.evidenceSnapshot.cadenceIdentity, 'cadence:fixture-1');
  for (const finding of model.findings.filter(
    (candidate) => 'evidenceIdentity' in candidate,
  )) {
    assert.equal(finding.evidenceIdentity, 'runtime-weights-cadence:fixture-1');
  }
  for (const finding of model.findings.filter(
    (candidate) =>
      candidate.kind === 'ThresholdChatterRisk' ||
      candidate.kind === 'MissingHysteresisOrPersistence',
  )) {
    assert.equal(finding.artifactIdentity, 'artifact:test-plan');
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
        sovereignAccount: sovereignAccount({
          $runtimeType: 'AccountId32',
          $hex: `0x${'11'.repeat(32)}`,
        }),
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
        sovereignAccount: sovereignAccount(marketAccount),
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
        producer: 'AxialRouterPreExecutionReserves',
        lifecycle: 'Active',
        evidence: {
          provenance: 'RuntimeDerived',
          identity: 'state:observation-fixture',
        },
        effectMatchers: [
          {
            actorId: 'market-actor',
            effectClasses: ['Swap'],
            evidenceIdentity: 'declaration:market-price-effect',
          },
        ],
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
        edge.family === 'Coordination' &&
        edge.provenance === 'ArtifactDerived' &&
        edge.evidenceIdentities.includes('state:finalized-fixture') &&
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

test('same asset in independent sovereign accounts never synthesizes shared coupling', () => {
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
        sovereignAccount: sovereignAccount(upstreamAccount),
        analysis: analysis({
          steps: [
            projectionStep({
              task: 'Transfer',
              reads: [native],
              writes: [native],
              recipients: [],
            }),
          ],
        }),
      },
      {
        id: 'downstream',
        sovereignAccount: sovereignAccount(downstreamAccount),
        analysis: analysis({
          steps: [
            projectionStep({
              task: 'Transfer',
              reads: [native],
              writes: [native],
              recipients: [],
            }),
          ],
        }),
      },
    ],
    observations: [],
  });
  assert.deepEqual(model.components, []);
  assert.equal(
    model.edges.filter((edge) => edge.kind === 'ActorSignal').length,
    0,
  );
  assert(
    model.edges
      .filter((edge) => edge.kind.startsWith('SharedAsset'))
      .every(
        (edge) =>
          edge.family === 'ResourceCoupling' &&
          edge.provenance === 'ArtifactDerived',
      ),
  );
  assert.equal(
    model.findings.filter(
      (finding) =>
        finding.kind === 'SharedAssetCoupling' ||
        finding.kind === 'PotentialResourceContention',
    ).length,
    0,
  );
  const accountResources = model.nodes.filter(
    (node) => node.kind === 'Resource',
  );
  assert.equal(accountResources.length, 2);
  assert(
    accountResources.every(
      (resource) => resource.resourceKind === 'AccountAsset',
    ),
  );
});

test('exact pool identity separates equal pairs and reports true shared contention', () => {
  const swapAnalysis = () =>
    analysis({
      steps: [
        projectionStep({ task: 'SwapIn', reads: [native], writes: [bldr] }),
      ],
    });
  const distinct = analyzeAaaFeedback({
    actors: [
      {
        id: 'pool-a-actor',
        analysis: swapAnalysis(),
        exactResources: [exactResource('Pool', 'pool:a', 'Write')],
      },
      {
        id: 'pool-b-actor',
        analysis: swapAnalysis(),
        exactResources: [exactResource('Pool', 'pool:b', 'Write')],
      },
    ],
    observations: [],
  });
  assert.equal(
    distinct.findings.filter((finding) => finding.kind === 'SharedPoolCoupling')
      .length,
    0,
  );
  assert.deepEqual(
    distinct.nodes
      .filter(
        (node) => node.kind === 'Resource' && node.resourceKind === 'Pool',
      )
      .map((node) => node.resourceIdentity)
      .sort(),
    ['pool:a', 'pool:b'],
  );

  const shared = analyzeAaaFeedback({
    actors: [
      {
        id: 'writer-a',
        analysis: swapAnalysis(),
        exactResources: [exactResource('Pool', 'pool:shared', 'Write')],
      },
      {
        id: 'writer-b',
        analysis: swapAnalysis(),
        exactResources: [exactResource('Pool', 'pool:shared', 'Write')],
      },
    ],
    observations: [],
  });
  const sharedKinds = new Set(shared.findings.map((finding) => finding.kind));
  assert(sharedKinds.has('SharedPoolCoupling'));
  assert(sharedKinds.has('PotentialResourceContention'));
  assert.deepEqual(shared.components, []);
  assert(
    shared.edges
      .filter((edge) => edge.kind === 'ExactResourceWrite')
      .every(
        (edge) =>
          edge.family === 'ResourceCoupling' &&
          edge.provenance === 'RuntimeDerived' &&
          edge.evidenceIdentities.includes('state:finalized-fixture'),
      ),
  );
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
        producer: 'Unknown',
        lifecycle: 'Unknown',
        evidence: { provenance: 'Unknown', identity: null },
        effectMatchers: [],
      },
    ],
    parameterActuators: [
      {
        id: 'fee-rate',
        evidenceIdentity: 'declaration:fee-rate',
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
        producer: 'DeclaredExternal',
        lifecycle: 'Active',
        evidence: {
          provenance: 'Declared',
          identity: 'observation:external-release-signal',
        },
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

test('deactivated runtime observations preserve identity without causal recurrence', () => {
  const model = analyzeAaaFeedback({
    actors: [
      {
        id: 'market',
        analysis: analysis({
          triggerFeeds: [priceFeed],
          steps: [projectionStep({ task: 'SwapIn', writes: [bldr] })],
        }),
      },
    ],
    observations: [
      {
        id: 'deactivated-price',
        feed: priceFeed,
        producer: 'AxialRouterPreExecutionReserves',
        lifecycle: 'Deactivated',
        evidence: {
          provenance: 'RuntimeDerived',
          identity: 'state:observation-fixture',
        },
        effectMatchers: [
          {
            actorId: 'market',
            effectClasses: ['Swap'],
            evidenceIdentity: 'declaration:market-price-effect',
          },
        ],
      },
    ],
  });
  assert.deepEqual(model.components, []);
  assert.equal(
    model.edges.filter((edge) => edge.kind === 'ActorEffectOnObservation')
      .length,
    0,
  );
  const observation = model.nodes.find((node) => node.kind === 'Observation');
  assert.equal(observation.provenance, 'Endogenous');
  assert.equal(observation.lifecycle, 'Deactivated');
});

test('timing and policy evidence rejects provenance and identity substitution', () => {
  const actor = {
    id: 'actor',
    analysis: analysis({ triggerFeeds: [priceFeed] }),
  };
  const observation = {
    id: 'price',
    feed: priceFeed,
    producer: 'AxialRouterPreExecutionReserves',
    lifecycle: 'Active',
    evidence: {
      provenance: 'RuntimeDerived',
      identity: 'state:observation-fixture',
    },
    effectMatchers: [],
  };
  const evidence = {
    identity: 'evidence:combined',
    runtimeIdentity: verifiedRuntimeIdentity,
    runtimeVerification: {
      status: 'Verified',
      observedIdentity: verifiedRuntimeIdentity,
      scheduler: schedulerEvidence,
      reasons: [],
    },
    weightIdentity: runtimeEvidence.weightIdentity,
    cadenceIdentity: 'cadence:one',
    estimatedDeliveryBlocks: 1,
    estimatedDeliveryEvidence: {
      provenance: 'RuntimeDerived',
      identity: 'evidence:combined',
    },
    observationCadences: [
      {
        observationId: 'price',
        minimumUpdateIntervalBlocks: 1,
        evidence: { provenance: 'RuntimeDerived', identity: 'cadence:one' },
      },
    ],
    actorPolicies: [
      {
        actorId: 'actor',
        gain: 'Unknown',
        gainEvidence: { provenance: 'Unknown', identity: null },
        reactiveIngressPriority: 'Unknown',
        reactiveIngressPriorityEvidence: {
          provenance: 'Unknown',
          identity: null,
        },
      },
    ],
  };
  const input = { actors: [actor], observations: [observation], evidence };
  assert.doesNotThrow(() => analyzeAaaFeedback(input));
  const mismatchEvidence = {
    ...evidence,
    runtimeIdentity: 'runtime:drifted',
    runtimeVerification: {
      status: 'EvidenceMismatch',
      observedIdentity: 'runtime:drifted',
      scheduler: schedulerEvidence,
      reasons: ['runtime code mismatch'],
    },
  };
  const mismatchModel = analyzeAaaFeedback({
    ...input,
    evidence: mismatchEvidence,
  });
  assert.equal(mismatchModel.evidenceStatus, 'EvidenceMismatch');
  assert.equal(
    mismatchModel.findings.filter((finding) => 'evidenceIdentity' in finding)
      .length,
    0,
  );
  assert.throws(
    () =>
      analyzeAaaFeedback({
        ...input,
        evidence: {
          ...evidence,
          runtimeIdentity: 'runtime:forged',
          runtimeVerification: {
            status: 'Verified',
            observedIdentity: 'runtime:forged',
            scheduler: schedulerEvidence,
            reasons: [],
          },
        },
      }),
    /differs from generated truth/,
  );
  assert.throws(
    () =>
      analyzeAaaFeedback({
        ...input,
        evidence: {
          ...evidence,
          runtimeVerification: {
            ...evidence.runtimeVerification,
            scheduler: {
              ...schedulerEvidence,
              maxServiceUnitsPerBlock:
                schedulerEvidence.maxServiceUnitsPerBlock + 1,
            },
          },
        },
      }),
    /differs from generated truth/,
  );
  assert.throws(
    () =>
      analyzeAaaFeedback({
        ...input,
        evidence: {
          ...evidence,
          estimatedDeliveryEvidence: {
            provenance: 'Declared',
            identity: 'declaration:estimate',
          },
        },
      }),
    /Estimated delivery uses disallowed evidence provenance/,
  );
  assert.throws(
    () =>
      analyzeAaaFeedback({
        ...input,
        evidence: {
          ...evidence,
          observationCadences: [
            {
              ...evidence.observationCadences[0],
              evidence: {
                provenance: 'RuntimeDerived',
                identity: 'cadence:other',
              },
            },
          ],
        },
      }),
    /cadence evidence identity mismatch/,
  );
  assert.throws(
    () =>
      analyzeAaaFeedback({
        ...input,
        evidence: {
          ...evidence,
          actorPolicies: [
            {
              ...evidence.actorPolicies[0],
              gain: 'High',
              gainEvidence: { provenance: 'Unknown', identity: null },
            },
          ],
        },
      }),
    /gain evidence disagrees/,
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
        producer: 'AxialRouterPreExecutionReserves',
        lifecycle: 'Active',
        evidence: {
          provenance: 'RuntimeDerived',
          identity: 'state:observation-fixture',
        },
        effectMatchers: [
          {
            effectClasses: ['Swap'],
            evidenceIdentity: 'declaration:swap-price-effect',
          },
        ],
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
            producer: 'DeclaredExternal',
            evidence: {
              provenance: 'Declared',
              identity: 'declaration:external-observation',
            },
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
  assert.throws(
    () =>
      analyzeAaaFeedback({
        ...input,
        observations: [
          {
            ...input.observations[0],
            effectMatchers: [{ effectClasses: ['Swap'], evidenceIdentity: '' }],
          },
        ],
      }),
    /effect matcher evidence identity is required/,
  );
  assert.throws(
    () =>
      analyzeAaaFeedback({
        ...input,
        observations: [
          {
            ...input.observations[0],
            evidence: { provenance: 'Unknown', identity: null },
          },
        ],
      }),
    /uses disallowed evidence provenance/,
  );
  assert.throws(
    () =>
      analyzeAaaFeedback({
        actors: [
          {
            id: 'declared-account',
            analysis: analysis(),
            sovereignAccount: {
              value: {
                $runtimeType: 'AccountId32',
                $hex: `0x${'55'.repeat(32)}`,
              },
              evidence: {
                provenance: 'Declared',
                identity: 'declaration:account',
              },
            },
          },
        ],
        observations: [],
      }),
    /sovereign account uses disallowed evidence provenance/,
  );
  assert.throws(
    () =>
      analyzeAaaFeedback({
        actors: [
          {
            id: 'account-a',
            analysis: analysis(),
            sovereignAccount: sovereignAccount({ account: 'a' }),
          },
          {
            id: 'account-b',
            analysis: analysis(),
            sovereignAccount: {
              value: { account: 'b' },
              evidence: {
                provenance: 'RuntimeDerived',
                identity: 'state:other-finalized-fixture',
              },
            },
          },
        ],
        observations: [],
      }),
    /Runtime-derived actor resources must share one state identity/,
  );
  assert.throws(
    () =>
      analyzeAaaFeedback({
        actors: [
          {
            id: 'declared-pool',
            analysis: analysis(),
            exactResources: [
              {
                kind: 'Pool',
                identity: 'pool:declared',
                access: 'Read',
                evidence: {
                  provenance: 'Declared',
                  identity: 'declaration:pool',
                },
              },
            ],
          },
        ],
        observations: [],
      }),
    /Pool resource uses disallowed evidence provenance/,
  );
  const driftedAnalysis = analysis();
  driftedAnalysis.identity = {
    ...driftedAnalysis.identity,
    metadataHash: 'runtime:drifted-metadata',
  };
  assert.throws(
    () =>
      analyzeAaaFeedback({
        ...input,
        actors: [
          ...input.actors,
          { id: 'drifted-actor', analysis: driftedAnalysis },
        ],
      }),
    /share one runtime evidence context/,
  );
});
