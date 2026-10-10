/*
Domain: Actors current behavior episode
Owns: Bounded typed policy requirements and authenticated capped monitor projection.
Excludes: Policy compilation, observation history, state mutation, and admission.
*/
import {
  ACTOR_BEHAVIOR_METRICS,
  type ActorBehaviorAtom,
  type ActorBehaviorMetric,
} from './behavior-policy.ts';

export type ActorBehaviorEpisodeView = {
  episodeStartedAtGeneration: bigint;
  trackingMode: 'CurrentOutcome' | 'Accumulated';
  requirements: ActorBehaviorAtom[];
  monitor: {
    cappedValues: { metric: ActorBehaviorMetric; value: number }[];
    cleanCycle: {
      cycleNonce: bigint;
      possible: boolean;
      economicSuccess: boolean;
    } | null;
  } | null;
};

function object(value: unknown, label: string): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value))
    throw new Error(`${label} must be a runtime object`);
  return value as Record<string, unknown>;
}

function u32(value: unknown, label: string): number {
  if (
    typeof value !== 'number' ||
    !Number.isInteger(value) ||
    value < 0 ||
    value > 0xffff_ffff
  )
    throw new Error(`${label} must fit u32`);
  return value;
}

function positiveU64(value: unknown, label: string): bigint {
  if (
    typeof value !== 'bigint' ||
    value <= 0n ||
    value > 0xffff_ffff_ffff_ffffn
  )
    throw new Error(`${label} must be a positive u64`);
  return value;
}

function metric(value: unknown): ActorBehaviorMetric {
  const name = object(value, 'Behavior metric').type;
  if (!ACTOR_BEHAVIOR_METRICS.some((candidate) => candidate === name))
    throw new Error(`Unsupported runtime behavior metric ${String(name)}`);
  return name as ActorBehaviorMetric;
}

function boundedArray(value: unknown, label: string): unknown[] {
  if (!Array.isArray(value) || value.length > ACTOR_BEHAVIOR_METRICS.length)
    throw new Error(`${label} exceeds the metric bound`);
  return value;
}

export function projectActorBehaviorEpisode(
  value: unknown,
): ActorBehaviorEpisodeView | null {
  if (value === undefined) return null;
  const episode = object(value, 'Behavior episode');
  const mode = object(episode.tracking_mode, 'Behavior tracking mode').type;
  if (mode !== 'CurrentOutcome' && mode !== 'Accumulated')
    throw new Error(
      `Unsupported runtime behavior tracking mode ${String(mode)}`,
    );
  const requirements = boundedArray(
    episode.requirements,
    'Behavior requirements',
  ).map((value) => {
    const fields = object(value, 'Behavior requirement');
    const threshold = u32(fields.threshold, 'Behavior threshold');
    if (threshold === 0) throw new Error('Behavior threshold must be positive');
    return { metric: metric(fields.metric), threshold };
  });
  if (requirements.length === 0)
    throw new Error('Behavior requirements must not be empty');
  if (
    mode === 'CurrentOutcome' &&
    requirements.some((requirement) => requirement.threshold !== 1)
  ) {
    throw new Error('Current-outcome requirements must have threshold one');
  }
  let previous = -1;
  for (const requirement of requirements) {
    const index = ACTOR_BEHAVIOR_METRICS.indexOf(requirement.metric);
    if (index <= previous)
      throw new Error(
        'Behavior requirements must have unique canonical metric order',
      );
    previous = index;
  }
  const requiresCleanCycle = requirements.some(
    ({ metric }) => metric === 'CycleCleanTotal',
  );
  let monitor: ActorBehaviorEpisodeView['monitor'] = null;
  if (episode.monitor !== undefined) {
    const fields = object(episode.monitor, 'Behavior monitor');
    const cappedValues = boundedArray(
      fields.capped_values,
      'Behavior capped values',
    ).map((value) => {
      const fields = object(value, 'Behavior capped value');
      return {
        metric: metric(fields.metric),
        value: u32(fields.value, 'Behavior capped value'),
      };
    });
    if (
      cappedValues.length !== (mode === 'Accumulated' ? requirements.length : 0)
    )
      throw new Error('Behavior counter slots do not match the tracking mode');
    cappedValues.forEach((counter, index) => {
      if (
        counter.metric !== requirements[index].metric ||
        counter.value > requirements[index].threshold
      )
        throw new Error('Behavior capped value does not match its requirement');
    });
    let cleanCycle: NonNullable<
      ActorBehaviorEpisodeView['monitor']
    >['cleanCycle'] = null;
    if (fields.clean_cycle !== undefined) {
      if (!requiresCleanCycle) throw new Error('Unexpected clean-Cycle state');
      const clean = object(fields.clean_cycle, 'Clean-Cycle state');
      const candidate = object(clean.candidate, 'Clean-Cycle candidate');
      if (
        typeof candidate.possible !== 'boolean' ||
        typeof candidate.economic_success !== 'boolean'
      )
        throw new Error('Clean-Cycle flags must be boolean');
      cleanCycle = {
        cycleNonce: positiveU64(clean.cycle_nonce, 'Clean-Cycle nonce'),
        possible: candidate.possible,
        economicSuccess: candidate.economic_success,
      };
    }
    monitor = { cappedValues, cleanCycle };
  }
  if ((mode === 'Accumulated' || requiresCleanCycle) !== (monitor !== null))
    throw new Error(
      'Behavior monitor presence does not match its requirements',
    );
  return {
    episodeStartedAtGeneration: positiveU64(
      episode.episode_started_at_generation,
      'Behavior episode generation',
    ),
    trackingMode: mode,
    requirements,
    monitor,
  };
}
