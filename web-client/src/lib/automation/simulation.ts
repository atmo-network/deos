/*
Domain: AAA control-plane local simulation
Owns: Honest adapter-local partial-execution projection, task rollback, scalar Continuation outcomes, and donation sensitivity.
Excludes: Runtime-Wasm execution, chain queries, scheduler prediction, signing, submission, and persistence.
Zone: Automation domain capability; matching-runtime truth requires a separate Wasm/state-proof adapter.
*/
import type { AaaPlanArtifact, AaaPlanHex } from './plan-artifact';

export type AaaLocalSimulationProvenance = {
  truth: 'AdapterLocalProjection';
  planId: AaaPlanHex;
  blockHash: AaaPlanHex;
  metadataHash: AaaPlanHex;
  model: string;
  modelVersion: string;
};

export type AaaStepErrorPolicy =
  | { type: 'AbortCycle' }
  | { type: 'ContinueNextStep' }
  | { type: 'RetryLater'; maxAttempts: number };

export type AaaLocalConditionSet<Condition> =
  | { type: 'Always' }
  | { type: 'All' | 'Any'; conditions: Condition[] };

export type AaaLocalStep<Condition> = {
  stepIndex: number;
  conditionSet: AaaLocalConditionSet<Condition>;
  taskControl: 'Execute' | 'StopCycle';
  onError: AaaStepErrorPolicy;
};

export type AaaLocalConditionOutcome =
  | { kind: 'Value'; value: boolean }
  | { kind: 'Error'; retry: 'Temporary' | 'Permanent'; error: string };

export type AaaLocalStepOutcome =
  | { kind: 'Executed' }
  | { kind: 'Stopped' }
  | { kind: 'SkippedCondition' }
  | { kind: 'SkippedResolution' }
  | { kind: 'FundingUnavailable' }
  | { kind: 'Failed'; retry: 'Temporary' | 'Permanent'; error: string };

export type AaaLocalSimulationCounts = {
  executedSteps: number;
  skippedConditions: number;
  skippedResolution: number;
  skippedFundingUnavailable: number;
  failedSteps: number;
};

export type AaaLocalSimulationJournalEntry = {
  stepIndex: number;
  outcome: AaaLocalStepOutcome;
  stateCommitted: boolean;
};

export type AaaLocalSimulationResult<State> = {
  provenance: AaaLocalSimulationProvenance;
  status: 'Completed' | 'Aborted' | 'Suspended' | 'Closed';
  closeReason: 'RetryAttemptsExhausted' | null;
  cycleNonce: bigint;
  attempt: number;
  startCursor: number;
  continuationCursor: number | null;
  unsuccessfulAttemptsAtCursor: number | null;
  finalizedThrough: number | null;
  state: State;
  cumulative: AaaLocalSimulationCounts;
  journal: AaaLocalSimulationJournalEntry[];
};

export type AaaDonationSurface = {
  stepIndex: number;
  surface: string;
  resolution:
    | 'Fixed'
    | 'AllBalance'
    | 'PercentageOfCurrent'
    | 'PercentageOfTrigger'
    | 'PercentageOfLastFunding';
  observation: 'ActorBalance' | 'ActorFunding' | 'AdapterState';
};

export type AaaDonationSensitivity = {
  stepIndex: number;
  surface: string;
  sensitivity:
    | 'InsensitiveFixedAmount'
    | 'BeforeStepResolution'
    | 'BeforeTriggerSnapshot'
    | 'BeforeFundingSnapshot'
    | 'BeforeAdapterObservation';
  reason: string;
};

const EMPTY_COUNTS: AaaLocalSimulationCounts = {
  executedSteps: 0,
  skippedConditions: 0,
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
  counts: AaaLocalSimulationCounts,
  key: keyof AaaLocalSimulationCounts,
) {
  const value = counts[key] + 1;
  if (!Number.isSafeInteger(value)) throw new Error(`${key} overflow`);
  counts[key] = value;
}

export function simulateAaaLocally<State, Condition>(input: {
  artifact: AaaPlanArtifact;
  blockHash: AaaPlanHex;
  model: string;
  modelVersion: string;
  cycleNonce: bigint;
  attempt: number;
  startCursor: number;
  unsuccessfulAttemptsAtCursor?: number;
  initialState: State;
  initialCounts?: AaaLocalSimulationCounts;
  steps: AaaLocalStep<Condition>[];
  evaluateCondition?: (
    condition: Condition,
    state: Readonly<State>,
  ) => AaaLocalConditionOutcome;
  runTask: (
    step: AaaLocalStep<Condition>,
    taskLocalState: State,
  ) => AaaLocalStepOutcome;
}): AaaLocalSimulationResult<State> {
  validateIndex(input.attempt, 'attempt');
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
        step.onError.maxAttempts > 0xffff_ffff)
    ) {
      throw new Error('RetryLater maxAttempts must be a nonzero u32');
    }
    if (
      step.conditionSet.type !== 'Always' &&
      step.conditionSet.conditions.length === 0
    ) {
      throw new Error(
        `${step.conditionSet.type} condition set must be non-empty`,
      );
    }
    if (
      step.conditionSet.type !== 'Always' &&
      input.evaluateCondition == null
    ) {
      throw new Error('Grouped conditions require an evaluator');
    }
  });

  let state = structuredClone(input.initialState);
  const cumulative = structuredClone(input.initialCounts ?? EMPTY_COUNTS);
  for (const key of Object.keys(cumulative) as Array<
    keyof AaaLocalSimulationCounts
  >) {
    validateIndex(cumulative[key], `initialCounts.${key}`);
  }
  const journal: AaaLocalSimulationJournalEntry[] = [];
  let finalizedThrough = input.startCursor === 0 ? null : input.startCursor - 1;

  for (let index = input.startCursor; index < input.steps.length; index += 1) {
    const step = input.steps[index];
    const taskLocalState = structuredClone(state);
    const outcome = (() => {
      if (step.conditionSet.type !== 'Always') {
        let truth = step.conditionSet.type === 'All';
        let firstError: Extract<
          AaaLocalConditionOutcome,
          { kind: 'Error' }
        > | null = null;
        for (const condition of step.conditionSet.conditions) {
          const current = input.evaluateCondition!(condition, state);
          if (current.kind === 'Error') firstError ??= current;
          else if (step.conditionSet.type === 'All') truth &&= current.value;
          else truth ||= current.value;
        }
        if (firstError != null) {
          return {
            kind: 'Failed',
            retry: firstError.retry,
            error: firstError.error,
          } as const;
        }
        if (!truth) return { kind: 'SkippedCondition' } as const;
      }
      if (step.taskControl === 'StopCycle') return { kind: 'Stopped' } as const;
      return input.runTask(step, taskLocalState);
    })();
    let stateCommitted = false;
    switch (outcome.kind) {
      case 'Executed':
        state = taskLocalState;
        stateCommitted = true;
        increment(cumulative, 'executedSteps');
        break;
      case 'Stopped':
        break;
      case 'SkippedCondition':
        increment(cumulative, 'skippedConditions');
        break;
      case 'SkippedResolution':
        increment(cumulative, 'skippedResolution');
        break;
      case 'FundingUnavailable':
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

    if (outcome.kind === 'Stopped') {
      return {
        provenance: provenance(input),
        status: 'Completed',
        closeReason: null,
        cycleNonce: input.cycleNonce,
        attempt: input.attempt,
        startCursor: input.startCursor,
        continuationCursor: null,
        unsuccessfulAttemptsAtCursor: null,
        finalizedThrough: index,
        state,
        cumulative,
        journal,
      };
    }

    const retryable =
      outcome.kind === 'FundingUnavailable' ||
      (outcome.kind === 'Failed' && outcome.retry === 'Temporary');
    if (step.onError.type === 'RetryLater' && retryable) {
      const nextUnsuccessfulAttempts =
        index === input.startCursor
          ? Math.min(priorUnsuccessfulAttempts + 1, 0xffff_ffff)
          : 1;
      const exhausted = nextUnsuccessfulAttempts >= step.onError.maxAttempts;
      return {
        provenance: provenance(input),
        status: exhausted ? 'Closed' : 'Suspended',
        closeReason: exhausted ? 'RetryAttemptsExhausted' : null,
        cycleNonce: input.cycleNonce,
        attempt: input.attempt,
        startCursor: input.startCursor,
        continuationCursor: exhausted ? null : index,
        unsuccessfulAttemptsAtCursor: exhausted
          ? null
          : nextUnsuccessfulAttempts,
        finalizedThrough: index === 0 ? null : index - 1,
        state,
        cumulative,
        journal,
      };
    }
    if (outcome.kind === 'Failed' && step.onError.type !== 'ContinueNextStep') {
      return {
        provenance: provenance(input),
        status: 'Aborted',
        closeReason: null,
        cycleNonce: input.cycleNonce,
        attempt: input.attempt,
        startCursor: input.startCursor,
        continuationCursor: null,
        unsuccessfulAttemptsAtCursor: null,
        finalizedThrough: index,
        state,
        cumulative,
        journal,
      };
    }
    finalizedThrough = index;
  }

  return {
    provenance: provenance(input),
    status: 'Completed',
    closeReason: null,
    cycleNonce: input.cycleNonce,
    attempt: input.attempt,
    startCursor: input.startCursor,
    continuationCursor: null,
    unsuccessfulAttemptsAtCursor: null,
    finalizedThrough,
    state,
    cumulative,
    journal,
  };
}

function provenance(input: {
  artifact: AaaPlanArtifact;
  blockHash: AaaPlanHex;
  model: string;
  modelVersion: string;
}): AaaLocalSimulationProvenance {
  return {
    truth: 'AdapterLocalProjection',
    planId: input.artifact.planId,
    blockHash: input.blockHash,
    metadataHash: input.artifact.metadataHash,
    model: input.model,
    modelVersion: input.modelVersion,
  };
}

export function classifyAaaDonationSensitivity(
  surfaces: AaaDonationSurface[],
): AaaDonationSensitivity[] {
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
    if (surface.resolution === 'PercentageOfTrigger') {
      return {
        stepIndex: surface.stepIndex,
        surface: surface.surface,
        sensitivity: 'BeforeTriggerSnapshot',
        reason:
          'Actor balance changes can affect the captured trigger snapshot, but not its persisted value.',
      };
    }
    if (surface.resolution === 'PercentageOfLastFunding') {
      return {
        stepIndex: surface.stepIndex,
        surface: surface.surface,
        sensitivity: 'BeforeFundingSnapshot',
        reason:
          'Funding included before batch promotion can affect the last-funding snapshot.',
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
