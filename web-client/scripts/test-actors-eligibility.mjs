/*
Domain: Actors eligibility projection validation
Owns: Read-only runtime `ActorEligibilityApi` signature evidence and canonical domain projection fixtures.
Excludes: Runtime execution, chain access, scheduler semantics, and plan authoring.
Zone: Web-client validation entrypoint; imports the automation eligibility contract and metadata builders.
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
import test from 'node:test';

import { readActorsTestMetadata } from './actors-test-metadata.mjs';

import {
  ACTORS_ELIGIBILITY_RUNTIME_API,
  ACTORS_ELIGIBILITY_RUNTIME_API_VERSION,
  ACTOR_CLOSE_REASONS,
  projectActorEligibility,
} from '../src/lib/automation/eligibility.ts';

const metadataBytes = await readActorsTestMetadata();

function eligibilityMethod() {
  const metadata = unifyMetadata(decAnyMetadata(metadataBytes));
  const apis = metadata.apis.filter(
    (candidate) => candidate.name === 'ActorEligibilityApi',
  );
  assert.equal(apis.length, 1, 'metadata must expose ActorEligibilityApi once');
  assert.equal(
    apis[0].version,
    ACTORS_ELIGIBILITY_RUNTIME_API_VERSION,
    'ActorEligibilityApi version must match the client contract',
  );
  const methods = apis[0].methods.filter(
    (candidate) => candidate.name === 'actor_eligibility',
  );
  assert.equal(methods.length, 1, 'actor_eligibility must appear once');
  assert.deepEqual(
    methods[0].inputs.map((input) => input.name),
    ['actor_id'],
    'actor_eligibility must carry exactly the canonical actor_id input',
  );
  return { metadata, method: methods[0] };
}

function encodeProjection(value) {
  const { metadata, method } = eligibilityMethod();
  const codec = getDynamicBuilder(getLookupFn(metadata)).buildDefinition(
    method.output,
  );
  const encoded = codec.enc(value);
  const decoded = codec.dec(encoded);
  assert.deepEqual(
    encoded,
    codec.enc(decoded),
    'encoded eligibility Result must round-trip canonically',
  );
  return decoded;
}

test('runtime metadata omits the retired Trigger-state bond quote', () => {
  const { metadata } = eligibilityMethod();
  const api = metadata.apis.find(
    (candidate) => candidate.name === 'ActorEligibilityApi',
  );
  assert.equal(
    api.methods.some((candidate) => candidate.name === 'trigger_state_bond'),
    false,
  );
});

test('terminal and Park reasons exactly match the selected runtime metadata', () => {
  const { metadata } = eligibilityMethod();
  const closeReason = metadata.lookup.find(
    (node) =>
      node.path.join('::') ===
      'pallet_deos_actors::types::lifecycle::CloseReason',
  );
  assert(closeReason, 'CloseReason must remain reachable from Actors metadata');
  assert.deepEqual(
    closeReason.def.value.map((variant) => variant.name),
    ACTOR_CLOSE_REASONS,
  );
  const parkReason = metadata.lookup.find(
    (node) =>
      node.path.join('::') ===
      'pallet_deos_actors::types::lifecycle::ParkNegativeReason',
  );
  assert(parkReason, 'Park reason must remain reachable from Actors metadata');
  assert.deepEqual(
    parkReason.def.value.map(({ name, index }) => ({ name, index })),
    [{ name: 'ParkedBalanceBelowThreshold', index: 0 }],
  );
});

test('projectActorEligibility projects canonical active classification', () => {
  const decoded = encodeProjection({
    success: true,
    value: {
      type: 'Active',
      value: {
        trigger: { type: 'Cadenced', value: { every_ticks: 120n } },
        pending_signal: true,
        financial_wait: undefined,
        residence: { type: 'Service', value: { type: 'Pending' } },
        eligibility: {
          terminal_reason: undefined,
          execution_phase: { type: 'WaitingCadenceTick', value: 42n },
        },
      },
    },
  });
  assert.deepEqual(projectActorEligibility(decoded), {
    type: 'Active',
    trigger: { type: 'Cadenced', everyTicks: 120n },
    pendingSignal: true,
    financialWait: null,
    behaviorEpisode: null,
    residence: { type: 'Service', kind: 'Pending' },
    terminalReason: null,
    executionPhase: { type: 'WaitingCadenceTick', tick: 42 },
  });
});

test('projectActorEligibility preserves AtTime one-shot consumption', () => {
  const decoded = encodeProjection({
    success: true,
    value: {
      type: 'Active',
      value: {
        trigger: {
          type: 'AtTime',
          value: { after_ticks: 20n, consumed: true },
        },
        pending_signal: false,
        financial_wait: undefined,
        residence: { type: 'Disabled', value: undefined },
        eligibility: {
          terminal_reason: undefined,
          execution_phase: { type: 'WaitingSignal', value: undefined },
        },
      },
    },
  });
  assert.deepEqual(projectActorEligibility(decoded), {
    type: 'Active',
    trigger: { type: 'AtTime', afterTicks: 20n, consumed: true },
    pendingSignal: false,
    financialWait: null,
    behaviorEpisode: null,
    residence: { type: 'Disabled' },
    terminalReason: null,
    executionPhase: { type: 'WaitingSignal' },
  });
});

test('canonical residence SCALE payloads preserve Deadline and Park evidence', () => {
  const fixtures = [
    [
      { type: 'Deadline', value: { type: 'Block', value: 31 } },
      { type: 'DeadlineBlock', block: 31 },
    ],
    [
      {
        type: 'Deadline',
        value: { type: 'Tick', value: 18_446_744_073_709_551_615n },
      },
      { type: 'DeadlineTick', tick: 18_446_744_073_709_551_615n },
    ],
    [
      {
        type: 'Parked',
        value: {
          reason: { type: 'ParkedBalanceBelowThreshold' },
          review_at: 104,
        },
      },
      { type: 'Parked', reason: 'ParkedBalanceBelowThreshold', reviewAt: 104 },
    ],
    [
      {
        type: 'Parked',
        value: {
          reason: { type: 'ParkedBalanceBelowThreshold' },
          review_at: undefined,
        },
      },
      { type: 'Parked', reason: 'ParkedBalanceBelowThreshold', reviewAt: null },
    ],
  ];
  for (const [residence, expected] of fixtures) {
    const decoded = encodeProjection({
      success: true,
      value: {
        type: 'Active',
        value: {
          trigger: { type: 'Manual', value: undefined },
          pending_signal: false,
          financial_wait: undefined,
          residence,
          eligibility: {
            terminal_reason: undefined,
            execution_phase: { type: 'WaitingSignal', value: undefined },
          },
        },
      },
    });
    assert.deepEqual(projectActorEligibility(decoded).residence, expected);
  }
});

test('financial wait SCALE payload retains original unpaid reason and grace', () => {
  const decoded = encodeProjection({
    success: true,
    value: {
      type: 'Active',
      value: {
        trigger: {
          type: 'AtTime',
          value: { after_ticks: 20n, consumed: false },
        },
        pending_signal: false,
        residence: {
          type: 'Deadline',
          value: { type: 'Tick', value: 4_838_421n },
        },
        financial_wait: {
          unpaid_service: { type: 'AtTimeTrigger', value: { due_tick: 21n } },
          unpaid_liability: 300n,
          current_service_spendability: 299n,
          funding_shortfall: 1n,
          entered_at_millis: 10_500n,
          expiry_tick: 4_838_421n,
        },
        eligibility: {
          terminal_reason: undefined,
          execution_phase: { type: 'WaitingCadenceTick', value: 21n },
        },
      },
    },
  });
  assert.deepEqual(projectActorEligibility(decoded).financialWait, {
    unpaidService: { type: 'AtTimeTrigger', dueTick: 21n },
    unpaidLiability: 300n,
    currentServiceSpendability: 299n,
    fundingShortfall: 1n,
    enteredAtMillis: 10_500n,
    expiryTick: 4_838_421n,
  });
});

test('financial snapshots retain exact u128 debt and reject inconsistent shortfalls', () => {
  const sources = [
    { type: 'AtTimeTrigger', value: { due_tick: 21n } },
    { type: 'PipelineOpening', value: { cycle_nonce: 2n } },
    { type: 'RunningAction', value: { cycle_nonce: 2n, cursor: 3 } },
    {
      type: 'SuspendedAction',
      value: {
        cycle_nonce: 2n,
        cursor: 3,
        unsuccessful_attempts_at_cursor: 4,
      },
    },
  ];
  const debt = (1n << 100n) + 1n;
  for (const unpaid_service of sources) {
    for (const spendable of [0n, debt - 1n, debt, (1n << 128n) - 1n]) {
      const shortfall = spendable < debt ? debt - spendable : 0n;
      const decoded = encodeProjection({
        success: true,
        value: {
          type: 'Active',
          value: {
            trigger: { type: 'Manual' },
            pending_signal: false,
            residence: {
              type: 'Service',
              value: { type: 'Pending' },
            },
            financial_wait: {
              unpaid_service,
              unpaid_liability: debt,
              current_service_spendability: spendable,
              funding_shortfall: shortfall,
              entered_at_millis: 10_500n,
              expiry_tick: 4_838_421n,
            },
            eligibility: {
              terminal_reason: undefined,
              execution_phase: { type: 'WaitingSignal' },
            },
          },
        },
      });
      const view = projectActorEligibility(decoded);
      assert.equal(view.financialWait.unpaidLiability, debt);
      assert.equal(view.financialWait.currentServiceSpendability, spendable);
      assert.equal(view.financialWait.fundingShortfall, shortfall);
      assert.equal(view.executionPhase.type, 'WaitingSignal');
      if (unpaid_service.type === 'SuspendedAction') {
        assert.equal(
          view.financialWait.unpaidService.unsuccessfulAttemptsAtCursor,
          4,
        );
      }
      for (const fields of [
        { unpaid_liability: 0n },
        { unpaid_liability: 1n << 128n },
        { current_service_spendability: -1n },
        { current_service_spendability: 1n << 128n },
        { funding_shortfall: shortfall + 1n },
        { funding_shortfall: undefined },
      ]) {
        const invalid = structuredClone(decoded);
        Object.assign(invalid.value.value.financial_wait, fields);
        assert.throws(() => projectActorEligibility(invalid));
      }
    }
  }
});

test('behavior episode SCALE payload distinguishes stateless, accumulated and clean-only state', () => {
  const requirement = (metric, threshold) => ({
    metric: { type: metric },
    threshold,
  });
  const fixtures = [
    {
      tracking_mode: { type: 'CurrentOutcome' },
      requirements: [requirement('ActionSuccessTotal', 1)],
      monitor: undefined,
    },
    {
      tracking_mode: { type: 'Accumulated' },
      requirements: [requirement('ActionSuccessTotal', 2)],
      monitor: {
        capped_values: [{ metric: { type: 'ActionSuccessTotal' }, value: 2 }],
        clean_cycle: undefined,
      },
    },
    {
      tracking_mode: { type: 'CurrentOutcome' },
      requirements: [requirement('CycleCleanTotal', 1)],
      monitor: {
        capped_values: [],
        clean_cycle: {
          cycle_nonce: 3n,
          candidate: { possible: true, economic_success: false },
        },
      },
    },
  ];
  for (const fixture of fixtures) {
    const decoded = encodeProjection({
      success: true,
      value: {
        type: 'Active',
        value: {
          trigger: { type: 'Manual' },
          pending_signal: false,
          residence: { type: 'Disabled' },
          financial_wait: undefined,
          behavior_episode: { episode_started_at_generation: 7n, ...fixture },
          eligibility: {
            terminal_reason: undefined,
            execution_phase: { type: 'Paused' },
          },
        },
      },
    });
    const view = projectActorEligibility(decoded).behaviorEpisode;
    assert.equal(view.episodeStartedAtGeneration, 7n);
    assert.equal(view.trackingMode, fixture.tracking_mode.type);
    assert.deepEqual(
      view.requirements,
      fixture.requirements.map(({ metric, threshold }) => ({
        metric: metric.type,
        threshold,
      })),
    );
    assert.equal(view.monitor === null, fixture.monitor === undefined);
    if (view.monitor) {
      assert.deepEqual(
        view.monitor.cappedValues,
        fixture.monitor.capped_values.map(({ metric, value }) => ({
          metric: metric.type,
          value,
        })),
      );
      if (fixture.monitor.clean_cycle) {
        assert.deepEqual(view.monitor.cleanCycle, {
          cycleNonce: 3n,
          possible: true,
          economicSuccess: false,
        });
      } else assert.equal(view.monitor.cleanCycle, null);
    }
  }
});

test('projectActorEligibility preserves absence, dormancy, terminal reason, and execution phase', () => {
  for (const type of ['NotRegistered', 'Dormant']) {
    const decoded = encodeProjection({
      success: true,
      value: { type, value: undefined },
    });
    assert.deepEqual(projectActorEligibility(decoded), { type });
  }
  const active = encodeProjection({
    success: true,
    value: {
      type: 'Active',
      value: {
        trigger: { type: 'Manual', value: undefined },
        pending_signal: false,
        financial_wait: undefined,
        residence: { type: 'Disabled', value: undefined },
        eligibility: {
          terminal_reason: { type: 'WindowExpired', value: undefined },
          execution_phase: { type: 'GlobalCircuitBreaker', value: undefined },
        },
      },
    },
  });
  assert.deepEqual(projectActorEligibility(active), {
    type: 'Active',
    trigger: { type: 'Manual' },
    pendingSignal: false,
    financialWait: null,
    behaviorEpisode: null,
    residence: { type: 'Disabled' },
    terminalReason: 'WindowExpired',
    executionPhase: { type: 'GlobalCircuitBreaker' },
  });
});

test('projectActorEligibility rejects a typed runtime failure honestly', () => {
  const decoded = encodeProjection({
    success: false,
    value: { type: 'RunInvariant' },
  });
  assert.throws(() => projectActorEligibility(decoded), /RunInvariant/);
});

test('projectActorEligibility rejects unknown runtime variants and malformed results', () => {
  assert.throws(
    () =>
      projectActorEligibility({
        success: true,
        value: { type: 'Mystery' },
      }),
    /Unsupported runtime eligibility Mystery/,
  );
  assert.throws(
    () =>
      projectActorEligibility({
        success: true,
        value: {
          type: 'Active',
          value: {
            trigger: { type: 'Manual' },
            pending_signal: false,
            financial_wait: undefined,
            residence: { type: 'Disabled' },
            eligibility: {
              terminal_reason: { type: 'Mystery' },
              execution_phase: { type: 'Ready' },
            },
          },
        },
      }),
    /Unsupported runtime close reason Mystery/,
  );
  assert.throws(
    () =>
      projectActorEligibility({
        success: true,
        value: {
          type: 'Active',
          value: {
            trigger: { type: 'Manual' },
            pending_signal: false,
            financial_wait: undefined,
            residence: { type: 'Disabled' },
            eligibility: {
              terminal_reason: undefined,
              execution_phase: { type: 'Mystery' },
            },
          },
        },
      }),
    /Unsupported runtime execution phase Mystery/,
  );
  assert.throws(
    () =>
      projectActorEligibility({
        success: true,
        value: {
          type: 'Active',
          value: {
            trigger: { type: 'ObservationCrossing', value: {} },
            pending_signal: false,
            financial_wait: undefined,
            residence: { type: 'Disabled' },
            eligibility: {
              terminal_reason: undefined,
              execution_phase: { type: 'Ready' },
            },
          },
        },
      }),
    /Unsupported runtime trigger activation ObservationCrossing/,
  );
  assert.throws(
    () => projectActorEligibility({ success: 'maybe' }),
    /SCALE Result/,
  );
});
