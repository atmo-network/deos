/*
Domain: Actors control-plane local simulation
Owns: Honest adapter-local partial-execution projection, task rollback, scalar Continuation outcomes, and donation sensitivity.
Excludes: Runtime-Wasm execution, chain queries, scheduler prediction, signing, submission, and persistence.
Zone: Automation domain capability; matching-runtime truth requires a separate Wasm/state-proof adapter.
*/
import { ACTORS_MAX_RETRY_ATTEMPTS } from './actors-protocol-bounds.ts';
import {
  type ActorBehaviorLimits,
  type ActorBehaviorMetric,
  type ActorCloseWhen,
  validateActorCloseWhen,
} from './behavior-policy.ts';
import type {
  ActorContractArtifact,
  ActorContractHex,
} from './contract-artifact';

export type ActorLocalSimulationProvenance = {
  truth: 'AdapterLocalProjection';
  contractId: ActorContractHex;
  blockHash: ActorContractHex;
  metadataHash: ActorContractHex;
  model: string;
  modelVersion: string;
};

export type ActorStepErrorPolicy =
  | { type: 'AbortCycle' }
  | { type: 'ContinueNextStep' }
  | { type: 'RetryLater'; maxAttempts: number };

export type ActorLocalPrecondition<Predicate> = {
  clauses: Predicate[][];
};

export type ActorLocalStep<Predicate> = {
  stepIndex: number;
  precondition: ActorLocalPrecondition<Predicate> | null;
  taskControl: 'Execute' | 'StopCycle';
  onError: ActorStepErrorPolicy;
};

export type ActorLocalPredicateOutcome =
  | { kind: 'Value'; value: boolean }
  | { kind: 'Error'; retry: 'Temporary' | 'Permanent'; error: string };

export type ActorLocalStepOutcome =
  | { kind: 'Executed' }
  | { kind: 'Stopped' }
  | { kind: 'SkippedPrecondition' }
  | { kind: 'SkippedResolution' }
  | { kind: 'FundingUnavailable'; reason: 'ServiceFee' | 'TaskInput' }
  | { kind: 'Failed'; retry: 'Temporary' | 'Permanent'; error: string };

export type ActorLocalSimulationCounts = {
  executedSteps: number;
  committedEffectfulTasks: number;
  preconditionSkips: number;
  skippedResolution: number;
  skippedFundingUnavailable: number;
  failedSteps: number;
};

export type ActorLocalSimulationJournalEntry = {
  stepIndex: number;
  outcome: ActorLocalStepOutcome;
  stateCommitted: boolean;
};

export type ActorLocalBehaviorState = {
  contractId: ActorContractHex;
  policy: ActorCloseWhen;
  cappedMetrics: Partial<Record<ActorBehaviorMetric, number>>;
  cycle: { economicSuccess: boolean; disqualified: boolean };
};

export type ActorLocalSimulationResult<State> = {
  provenance: ActorLocalSimulationProvenance;
  status: 'Completed' | 'Failed' | 'Suspended' | 'Closed';
  closeReason: 'RetryAttemptsExhausted' | 'BehaviorPolicySatisfied' | null;
  cycleNonce: bigint;
  startCursor: number;
  runCursor: number | null;
  unsuccessfulAttemptsAtCursor: number | null;
  state: State;
  cumulative: ActorLocalSimulationCounts;
  behavior: ActorLocalBehaviorState | null;
  journal: ActorLocalSimulationJournalEntry[];
};

export type ActorDonationSurface = {
  stepIndex: number;
  surface: string;
  resolution: 'Fixed' | 'Percent';
  observation: 'ActorBalance' | 'AdapterState';
};

export type ActorDonationSensitivity = {
  stepIndex: number;
  surface: string;
  sensitivity:
    | 'InsensitiveFixedAmount'
    | 'BeforeStepResolution'
    | 'BeforeAdapterObservation';
  reason: string;
};

const EMPTY_COUNTS: ActorLocalSimulationCounts = {
  executedSteps: 0,
  committedEffectfulTasks: 0,
  preconditionSkips: 0,
  skippedResolution: 0,
  skippedFundingUnavailable: 0,
  failedSteps: 0,
};

function validateIndex(value: number, field: string) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${field} must be a non-negative safe integer`);
  }
}

function increment(
  counts: ActorLocalSimulationCounts,
  key: keyof ActorLocalSimulationCounts,
) {
  const value = counts[key] + 1;
  if (!Number.isSafeInteger(value)) throw new Error(`${key} overflow`);
  counts[key] = value;
}

function localBehaviorState(
  contractId: ActorContractHex,
  policy: ActorCloseWhen | null,
  initial: ActorLocalBehaviorState | null | undefined,
) {
  if (policy === null) {
    if (initial != null)
      throw new Error('Absent closeWhen cannot import a monitor');
    return null;
  }
  const caps: Partial<Record<ActorBehaviorMetric, number>> = {};
  for (const clause of policy.clauses) {
    for (const atom of clause) {
      caps[atom.metric] = Math.max(caps[atom.metric] ?? 0, atom.threshold);
    }
  }
  if (initial != null) {
    if (
      initial.contractId !== contractId ||
      JSON.stringify(initial.policy) !== JSON.stringify(policy) ||
      Object.keys(initial.cappedMetrics).length !== Object.keys(caps).length ||
      Object.entries(caps).some(([metric, cap]) => {
        const value = initial.cappedMetrics[metric as ActorBehaviorMetric];
        return (
          !Object.hasOwn(initial.cappedMetrics, metric) ||
          value == null ||
          !Number.isInteger(value) ||
          value < 0 ||
          value > cap
        );
      }) ||
      typeof initial.cycle.economicSuccess !== 'boolean' ||
      typeof initial.cycle.disqualified !== 'boolean'
    ) {
      throw new Error(
        'Local behavior history does not match its Contract and policy',
      );
    }
    return structuredClone(initial);
  }
  return {
    contractId,
    policy,
    cappedMetrics: Object.fromEntries(
      Object.keys(caps).map((metric) => [metric, 0]),
    ),
    cycle: { economicSuccess: false, disqualified: false },
  } satisfies ActorLocalBehaviorState;
}

function observeLocalBehavior(
  behavior: ActorLocalBehaviorState | null,
  observation: {
    precondition: 'Pass' | 'Reject' | null;
    action: 'Success' | 'Failure' | null;
    economicSuccess: boolean;
    disqualified: boolean;
    completed: boolean;
    planEnd: boolean;
    aborted: boolean;
  },
) {
  if (behavior === null) return false;
  const caps: Partial<Record<ActorBehaviorMetric, number>> = {};
  for (const clause of behavior.policy.clauses) {
    for (const atom of clause) {
      caps[atom.metric] = Math.max(caps[atom.metric] ?? 0, atom.threshold);
    }
  }
  const bump = (metric: ActorBehaviorMetric) => {
    if (caps[metric] != null) {
      behavior.cappedMetrics[metric] = Math.min(
        behavior.cappedMetrics[metric]! + 1,
        caps[metric]!,
      );
    }
  };
  const reset = (metric: ActorBehaviorMetric) => {
    if (caps[metric] != null) behavior.cappedMetrics[metric] = 0;
  };
  if (observation.precondition !== null) {
    const pass = observation.precondition === 'Pass';
    bump(pass ? 'PreconditionPassTotal' : 'PreconditionRejectTotal');
    bump(pass ? 'PreconditionPassStreak' : 'PreconditionRejectStreak');
    reset(pass ? 'PreconditionRejectStreak' : 'PreconditionPassStreak');
  }
  if (observation.action !== null) {
    const success = observation.action === 'Success';
    bump(success ? 'ActionSuccessTotal' : 'ActionFailureTotal');
    bump(success ? 'ActionSuccessStreak' : 'ActionFailureStreak');
    reset(success ? 'ActionFailureStreak' : 'ActionSuccessStreak');
  }
  behavior.cycle.economicSuccess ||= observation.economicSuccess;
  behavior.cycle.disqualified ||= observation.disqualified;
  if (observation.completed) {
    bump('CycleCompletedTotal');
    if (
      observation.planEnd &&
      behavior.cycle.economicSuccess &&
      !behavior.cycle.disqualified
    ) {
      bump('CycleCleanTotal');
    }
  }
  if (observation.completed || observation.aborted) {
    behavior.cycle = { economicSuccess: false, disqualified: false };
  }
  let satisfied = false;
  for (const clause of behavior.policy.clauses) {
    let matches = true;
    for (const atom of clause) {
      const reaches = behavior.cappedMetrics[atom.metric]! >= atom.threshold;
      matches = reaches && matches;
    }
    satisfied = matches || satisfied;
  }
  return satisfied;
}

export function simulateActorLocally<State, Predicate>(input: {
  artifact: ActorContractArtifact;
  blockHash: ActorContractHex;
  model: string;
  modelVersion: string;
  cycleNonce: bigint;
  startCursor: number;
  closeWhen?: ActorCloseWhen | null;
  behaviorLimits?: ActorBehaviorLimits;
  initialBehaviorState?: ActorLocalBehaviorState | null;
  unsuccessfulAttemptsAtCursor?: number;
  initialState: State;
  initialCounts?: ActorLocalSimulationCounts;
  steps: ActorLocalStep<Predicate>[];
  evaluatePredicate?: (
    predicate: Predicate,
    state: Readonly<State>,
  ) => ActorLocalPredicateOutcome;
  runTask: (
    step: ActorLocalStep<Predicate>,
    taskLocalState: State,
  ) => ActorLocalStepOutcome;
}): ActorLocalSimulationResult<State> {
  if ('completionPolicy' in input) {
    throw new Error(
      'Legacy completionPolicy is unsupported; author closeWhen explicitly',
    );
  }
  let policy: ActorCloseWhen | null = null;
  if (input.closeWhen != null) {
    if (input.behaviorLimits == null)
      throw new Error('Behavior limits are required');
    policy = validateActorCloseWhen(input.closeWhen, input.behaviorLimits);
  }
  const behavior = localBehaviorState(
    input.artifact.contractId,
    policy,
    input.initialBehaviorState,
  );
  if (
    (input.startCursor > 0 || (input.unsuccessfulAttemptsAtCursor ?? 0) > 0) &&
    policy?.clauses.some((clause) =>
      clause.some((atom) => atom.metric === 'CycleCleanTotal'),
    ) &&
    input.initialBehaviorState == null
  ) {
    throw new Error(
      'Clean-Cycle projection requires the committed prefix history',
    );
  }
  if (
    behavior?.cycle.economicSuccess &&
    !input.steps.some((step) => step.taskControl === 'Execute')
  ) {
    throw new Error(
      'Clean-Cycle history requires an economic Step in the plan',
    );
  }
  validateIndex(input.startCursor, 'startCursor');
  if (input.cycleNonce < 0n) throw new Error('cycleNonce must be non-negative');
  if (input.startCursor > input.steps.length) {
    throw new Error('startCursor exceeds the plan length');
  }
  const priorUnsuccessfulAttempts = input.unsuccessfulAttemptsAtCursor ?? 0;
  validateIndex(priorUnsuccessfulAttempts, 'unsuccessfulAttemptsAtCursor');
  if (priorUnsuccessfulAttempts > 0xffff_ffff) {
    throw new Error('unsuccessfulAttemptsAtCursor exceeds u32');
  }
  input.steps.forEach((step, index) => {
    if (step.stepIndex !== index) {
      throw new Error('steps must use contiguous ordered indices');
    }
    if (
      !['AbortCycle', 'ContinueNextStep', 'RetryLater'].includes(
        step.onError.type,
      )
    ) {
      throw new Error(`Unsupported error policy at step ${index}`);
    }
    if (
      input.artifact.mutability === 'Immutable' &&
      step.onError.type === 'RetryLater'
    ) {
      throw new Error('RetryLater remains Mutable-only');
    }
    if (
      step.onError.type === 'RetryLater' &&
      (!Number.isSafeInteger(step.onError.maxAttempts) ||
        step.onError.maxAttempts <= 0 ||
        step.onError.maxAttempts > ACTORS_MAX_RETRY_ATTEMPTS)
    ) {
      throw new Error(
        `RetryLater maxAttempts must be within 1..${ACTORS_MAX_RETRY_ATTEMPTS}`,
      );
    }
    if (
      step.precondition !== null &&
      (step.precondition.clauses.length === 0 ||
        step.precondition.clauses.some((clause) => clause.length === 0))
    ) {
      throw new Error('Precondition and every clause must be non-empty');
    }
    if (step.precondition !== null && input.evaluatePredicate == null) {
      throw new Error('Timed predicates require an evaluator');
    }
  });

  let state = structuredClone(input.initialState);
  const cumulative = structuredClone(input.initialCounts ?? EMPTY_COUNTS);
  for (const key of Object.keys(cumulative) as Array<
    keyof ActorLocalSimulationCounts
  >) {
    validateIndex(cumulative[key], `initialCounts.${key}`);
  }
  const journal: ActorLocalSimulationJournalEntry[] = [];
  const result = (
    status: ActorLocalSimulationResult<State>['status'],
    closeReason: ActorLocalSimulationResult<State>['closeReason'] = null,
    runCursor: number | null = null,
    unsuccessfulAttemptsAtCursor: number | null = null,
  ): ActorLocalSimulationResult<State> => ({
    provenance: provenance(input),
    status,
    closeReason,
    cycleNonce: input.cycleNonce,
    startCursor: input.startCursor,
    runCursor,
    unsuccessfulAttemptsAtCursor,
    state,
    cumulative,
    behavior,
    journal,
  });

  for (let index = input.startCursor; index < input.steps.length; index += 1) {
    const step = input.steps[index];
    const taskLocalState = structuredClone(state);
    let preconditionObservation: 'Pass' | 'Reject' | null = null;
    let invoked = false;
    const outcome = (() => {
      if (step.precondition !== null) {
        let expressionTruth = false;
        let firstError: Extract<
          ActorLocalPredicateOutcome,
          { kind: 'Error' }
        > | null = null;
        for (const clause of step.precondition.clauses) {
          let clauseTruth = true;
          for (const predicate of clause) {
            const current = input.evaluatePredicate!(predicate, state);
            if (current.kind === 'Error') {
              firstError ??= current;
              clauseTruth = false;
            } else {
              clauseTruth &&= current.value;
            }
          }
          expressionTruth ||= clauseTruth;
        }
        if (firstError != null) {
          return {
            kind: 'Failed',
            retry: firstError.retry,
            error: firstError.error,
          } as const;
        }
        preconditionObservation = expressionTruth ? 'Pass' : 'Reject';
        if (!expressionTruth) return { kind: 'SkippedPrecondition' } as const;
      }
      if (step.taskControl === 'StopCycle') return { kind: 'Stopped' } as const;
      invoked = true;
      return input.runTask(step, taskLocalState);
    })();
    let stateCommitted = false;
    if (outcome.kind === 'Stopped' && step.taskControl !== 'StopCycle') {
      throw new Error('Only StopCycle can produce a stopped outcome');
    }
    switch (outcome.kind) {
      case 'Executed':
        state = taskLocalState;
        stateCommitted = true;
        increment(cumulative, 'executedSteps');
        increment(cumulative, 'committedEffectfulTasks');
        break;
      case 'Stopped':
        break;
      case 'SkippedPrecondition':
        increment(cumulative, 'preconditionSkips');
        break;
      case 'SkippedResolution':
        increment(cumulative, 'skippedResolution');
        break;
      case 'FundingUnavailable':
        if (outcome.reason !== 'TaskInput') {
          throw new Error(
            'Local simulation cannot qualify service-fee insolvency or AwaitFunding',
          );
        }
        increment(cumulative, 'skippedFundingUnavailable');
        break;
      case 'Failed':
        if (
          !['Temporary', 'Permanent'].includes(outcome.retry) ||
          outcome.error.length === 0
        ) {
          throw new Error(
            'Failed outcomes require retry class and error label',
          );
        }
        increment(cumulative, 'failedSteps');
        break;
      default:
        throw new Error(`Unsupported step outcome at step ${index}`);
    }
    journal.push({ stepIndex: index, outcome, stateCommitted });
    const retryable =
      outcome.kind === 'FundingUnavailable' ||
      (outcome.kind === 'Failed' && outcome.retry === 'Temporary');
    const retrying = step.onError.type === 'RetryLater' && retryable;
    const attempts =
      index === input.startCursor
        ? Math.min(priorUnsuccessfulAttempts + 1, 0xffff_ffff)
        : 1;
    const exhausted =
      retrying &&
      step.onError.type === 'RetryLater' &&
      attempts >= step.onError.maxAttempts;
    const aborted =
      outcome.kind === 'Failed' &&
      step.onError.type !== 'ContinueNextStep' &&
      !retrying;
    const completed =
      outcome.kind === 'Stopped' ||
      (!retrying && !aborted && index + 1 === input.steps.length);
    const economic = step.taskControl === 'Execute';
    const satisfied = observeLocalBehavior(behavior, {
      precondition: preconditionObservation,
      action: !invoked
        ? null
        : outcome.kind === 'Executed'
          ? 'Success'
          : outcome.kind === 'Failed'
            ? 'Failure'
            : null,
      economicSuccess: economic && outcome.kind === 'Executed',
      disqualified:
        outcome.kind === 'Failed' ||
        (economic &&
          (outcome.kind === 'SkippedPrecondition' ||
            outcome.kind === 'SkippedResolution' ||
            (outcome.kind === 'FundingUnavailable' && !retrying))),
      completed,
      planEnd: completed && index + 1 === input.steps.length,
      aborted,
    });
    if (exhausted) return result('Closed', 'RetryAttemptsExhausted');
    if (satisfied) return result('Closed', 'BehaviorPolicySatisfied');
    if (outcome.kind === 'Stopped') return result('Completed');
    if (retrying) return result('Suspended', null, index, attempts);
    if (aborted) return result('Failed');
  }
  const zeroStepClose =
    input.steps.length === 0 &&
    observeLocalBehavior(behavior, {
      precondition: null,
      action: null,
      economicSuccess: false,
      disqualified: false,
      completed: true,
      planEnd: true,
      aborted: false,
    });
  return result(
    zeroStepClose ? 'Closed' : 'Completed',
    zeroStepClose ? 'BehaviorPolicySatisfied' : null,
  );
}

function provenance(input: {
  artifact: ActorContractArtifact;
  blockHash: ActorContractHex;
  model: string;
  modelVersion: string;
}): ActorLocalSimulationProvenance {
  return {
    truth: 'AdapterLocalProjection',
    contractId: input.artifact.contractId,
    blockHash: input.blockHash,
    metadataHash: input.artifact.metadataHash,
    model: input.model,
    modelVersion: input.modelVersion,
  };
}

export function classifyActorDonationSensitivity(
  surfaces: ActorDonationSurface[],
): ActorDonationSensitivity[] {
  return surfaces.map((surface) => {
    validateIndex(surface.stepIndex, 'stepIndex');
    if (surface.surface.length === 0) {
      throw new Error('Donation surfaces require a non-empty label');
    }
    if (surface.resolution === 'Fixed') {
      return {
        stepIndex: surface.stepIndex,
        surface: surface.surface,
        sensitivity: 'InsensitiveFixedAmount',
        reason: 'The fixed amount does not read a donated balance.',
      };
    }
    if (surface.observation === 'AdapterState') {
      return {
        stepIndex: surface.stepIndex,
        surface: surface.surface,
        sensitivity: 'BeforeAdapterObservation',
        reason:
          'External state can change before the adapter observes or quotes this surface.',
      };
    }
    return {
      stepIndex: surface.stepIndex,
      surface: surface.surface,
      sensitivity: 'BeforeStepResolution',
      reason:
        'Actor balance changes before this step can affect its live spendable-balance resolution.',
    };
  });
}
