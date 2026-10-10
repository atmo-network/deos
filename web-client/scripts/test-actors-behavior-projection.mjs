/*
Domain: Current behavior episode source conformance
Owns: Sparse/stateless distinction, bounded capped values, clean flags and malformed refusal.
Excludes: Runtime admission, policy compilation, exact lifetime statistics, and chain access.
*/
import assert from 'node:assert/strict';
import test from 'node:test';

import { projectActorBehaviorEpisode } from '../src/lib/automation/behavior-projection.ts';
import { projectActorEligibility } from '../src/lib/automation/eligibility.ts';

const requirement = (metric, threshold) => ({
  metric: { type: metric },
  threshold,
});
function episode(mode, requirements, monitor) {
  return {
    episode_started_at_generation: 7n,
    tracking_mode: { type: mode },
    requirements,
    monitor,
  };
}
const accumulated = () =>
  episode('Accumulated', [requirement('ActionSuccessTotal', 2)], {
    capped_values: [{ metric: { type: 'ActionSuccessTotal' }, value: 2 }],
    clean_cycle: undefined,
  });

test('behavior source distinguishes no policy, stateless policy and allocated monitor', () => {
  assert.equal(projectActorBehaviorEpisode(undefined), null);
  assert.deepEqual(
    projectActorBehaviorEpisode(
      episode(
        'CurrentOutcome',
        [requirement('ActionSuccessTotal', 1)],
        undefined,
      ),
    ),
    {
      episodeStartedAtGeneration: 7n,
      trackingMode: 'CurrentOutcome',
      requirements: [{ metric: 'ActionSuccessTotal', threshold: 1 }],
      monitor: null,
    },
  );
  assert.deepEqual(projectActorBehaviorEpisode(accumulated()).monitor, {
    cappedValues: [{ metric: 'ActionSuccessTotal', value: 2 }],
    cleanCycle: null,
  });
});

test('behavior source preserves allocated clean-only state without fabricated counters', () => {
  const input = episode('CurrentOutcome', [requirement('CycleCleanTotal', 1)], {
    capped_values: [],
    clean_cycle: {
      cycle_nonce: 3n,
      candidate: { possible: false, economic_success: true },
    },
  });
  assert.deepEqual(projectActorBehaviorEpisode(input).monitor, {
    cappedValues: [],
    cleanCycle: { cycleNonce: 3n, possible: false, economicSuccess: true },
  });
  input.monitor.clean_cycle = undefined;
  assert.deepEqual(projectActorBehaviorEpisode(input).monitor, {
    cappedValues: [],
    cleanCycle: null,
  });
});

test('eligibility carries behavior data independently of residence and financial status', () => {
  const view = projectActorEligibility({
    success: true,
    value: {
      type: 'Active',
      value: {
        trigger: { type: 'Manual' },
        pending_signal: false,
        residence: { type: 'Disabled' },
        behavior_episode: accumulated(),
        eligibility: {
          terminal_reason: undefined,
          execution_phase: { type: 'Paused' },
        },
      },
    },
  });
  assert.equal(view.behaviorEpisode.episodeStartedAtGeneration, 7n);
  assert.deepEqual(view.residence, { type: 'Disabled' });
  assert.equal(view.financialWait, null);
});

test('behavior source rejects missing, extra, mismatched or uncapped counter state', () => {
  const inputs = [
    episode('Accumulated', [requirement('ActionSuccessTotal', 2)], undefined),
    episode('CurrentOutcome', [requirement('ActionSuccessTotal', 1)], {
      capped_values: [],
      clean_cycle: undefined,
    }),
    episode('CurrentOutcome', [requirement('CycleCleanTotal', 1)], undefined),
    episode('Accumulated', [requirement('ActionSuccessTotal', 2)], {
      capped_values: [],
      clean_cycle: undefined,
    }),
  ];
  for (const value of [3, -1, 0x1_0000_0000]) {
    const input = accumulated();
    input.monitor.capped_values[0].value = value;
    inputs.push(input);
  }
  const foreign = accumulated();
  foreign.monitor.capped_values[0].metric.type = 'ActionFailureTotal';
  inputs.push(foreign);
  const duplicate = accumulated();
  duplicate.requirements.push(duplicate.requirements[0]);
  inputs.push(duplicate);
  const extraClean = accumulated();
  extraClean.monitor.clean_cycle = {
    cycle_nonce: 1n,
    candidate: { possible: true, economic_success: false },
  };
  inputs.push(extraClean);
  for (const input of inputs)
    assert.throws(() => projectActorBehaviorEpisode(input));
});

test('behavior source rejects unknown vocabulary, empty requirements and invalid scope', () => {
  for (const input of [
    null,
    episode('Unknown', [requirement('ActionSuccessTotal', 1)], undefined),
    episode('CurrentOutcome', [], undefined),
    episode('CurrentOutcome', [requirement('Unknown', 1)], undefined),
    episode(
      'CurrentOutcome',
      [requirement('ActionSuccessTotal', 2)],
      undefined,
    ),
    episode(
      'CurrentOutcome',
      [requirement('ActionSuccessTotal', 0)],
      undefined,
    ),
  ]) {
    assert.throws(() => projectActorBehaviorEpisode(input));
  }
  for (const generation of [0n, -1n, 0x1_0000_0000_0000_0000n]) {
    const input = accumulated();
    input.episode_started_at_generation = generation;
    assert.throws(() => projectActorBehaviorEpisode(input));
  }
});
