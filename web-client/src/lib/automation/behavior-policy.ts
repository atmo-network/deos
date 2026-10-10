/*
Domain: Actors authored outcome policies
Owns: Closed metric vocabulary and canonical bounded close_when validation.
Excludes: Runtime admission, fees, monitor storage, scheduling, and terminal execution.
*/
export const ACTOR_BEHAVIOR_METRICS = [
  'PreconditionPassTotal',
  'PreconditionPassStreak',
  'PreconditionRejectTotal',
  'PreconditionRejectStreak',
  'ActionSuccessTotal',
  'ActionSuccessStreak',
  'ActionFailureTotal',
  'ActionFailureStreak',
  'CycleCompletedTotal',
  'CycleCleanTotal',
] as const;

export type ActorBehaviorMetric = (typeof ACTOR_BEHAVIOR_METRICS)[number];
export const ACTOR_ON_UNFUNDED = ['Close', 'AwaitFunding'] as const;
export type ActorOnUnfunded = (typeof ACTOR_ON_UNFUNDED)[number];
export type ActorBehaviorAtom = {
  metric: ActorBehaviorMetric;
  threshold: number;
};
export type ActorCloseWhen = { clauses: ActorBehaviorAtom[][] };
export type ActorBehaviorLimits = {
  maxClauses: number;
  maxAtomsPerClause: number;
};

const U32_MAX = 0xffff_ffff;

function record(value: unknown, keys: string[], label: string) {
  if (value == null || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  const result = value as Record<string, unknown>;
  if (
    Object.keys(result).length !== keys.length ||
    keys.some((key) => !Object.hasOwn(result, key))
  ) {
    throw new Error(`${label} requires exactly ${keys.join(', ')}`);
  }
  return result;
}

function compareClauses(left: ActorBehaviorAtom[], right: ActorBehaviorAtom[]) {
  for (let index = 0; index < Math.min(left.length, right.length); index += 1) {
    const metric =
      ACTOR_BEHAVIOR_METRICS.indexOf(left[index].metric) -
      ACTOR_BEHAVIOR_METRICS.indexOf(right[index].metric);
    const threshold = left[index].threshold - right[index].threshold;
    if (metric !== 0 || threshold !== 0) return metric || threshold;
  }
  return left.length - right.length;
}

export function validateActorCloseWhen(
  value: unknown,
  limits: ActorBehaviorLimits,
): ActorCloseWhen {
  if (
    !Number.isInteger(limits.maxClauses) ||
    !Number.isInteger(limits.maxAtomsPerClause) ||
    limits.maxClauses <= 0 ||
    limits.maxAtomsPerClause <= 0 ||
    limits.maxClauses * limits.maxAtomsPerClause > U32_MAX
  ) {
    throw new Error('Behavior limits must be positive with a u32 product');
  }
  const expression = record(value, ['clauses'], 'closeWhen');
  if (
    !Array.isArray(expression.clauses) ||
    expression.clauses.length === 0 ||
    expression.clauses.length > limits.maxClauses
  ) {
    throw new Error(`closeWhen requires 1..${limits.maxClauses} clauses`);
  }
  const clauses = Array.from(expression.clauses, (clause, index) => {
    if (
      !Array.isArray(clause) ||
      clause.length === 0 ||
      clause.length > limits.maxAtomsPerClause
    ) {
      throw new Error(
        `closeWhen.clauses[${index}] requires 1..${limits.maxAtomsPerClause} atoms`,
      );
    }
    let previousMetric = -1;
    return Array.from(clause, (value, atomIndex) => {
      const atom = record(
        value,
        ['metric', 'threshold'],
        `closeWhen.clauses[${index}][${atomIndex}]`,
      );
      const metricIndex = ACTOR_BEHAVIOR_METRICS.indexOf(
        atom.metric as ActorBehaviorMetric,
      );
      if (metricIndex < 0) throw new Error('Unknown behavior metric');
      if (metricIndex <= previousMetric) {
        throw new Error(
          'Behavior atoms require unique metrics in declaration order',
        );
      }
      if (
        typeof atom.threshold !== 'number' ||
        !Number.isInteger(atom.threshold) ||
        atom.threshold <= 0 ||
        atom.threshold > U32_MAX
      ) {
        throw new Error('Behavior thresholds must be positive u32 integers');
      }
      previousMetric = metricIndex;
      return {
        metric: ACTOR_BEHAVIOR_METRICS[metricIndex],
        threshold: atom.threshold,
      };
    });
  });
  for (let index = 1; index < clauses.length; index += 1) {
    if (compareClauses(clauses[index - 1], clauses[index]) >= 0) {
      throw new Error('Behavior clauses require unique lexicographic order');
    }
  }
  for (let index = 0; index < clauses.length; index += 1) {
    for (let other = 0; other < clauses.length; other += 1) {
      if (
        index !== other &&
        clauses[other].every((weak) =>
          clauses[index].some(
            (strong) =>
              strong.metric === weak.metric &&
              strong.threshold >= weak.threshold,
          ),
        )
      ) {
        throw new Error('Behavior clauses must not imply another clause');
      }
    }
  }
  return { clauses };
}
