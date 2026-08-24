/*
Domain: Actors control-plane local simulation
Owns: Honest adapter-local partial-execution projection, task rollback, scalar Continuation outcomes, and donation sensitivity.
Excludes: Runtime-Wasm execution, chain queries, scheduler prediction, signing, submission, and persistence.
Zone: Automation domain capability; matching-runtime truth requires a separate Wasm/state-proof adapter.
*/
import { ACTORS_MAX_RETRY_ATTEMPTS } from './actors-protocol-bounds.ts';
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

export type ActorLocalTimedPredicate<Predicate> = {
  timing: 'Opening' | 'Current';
  predicate: Predicate;
};

export type ActorLocalPrecondition<Predicate> = {
  clauses: ActorLocalTimedPredicate<Predicate>[][];
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
  | { kind: 'FundingUnavailable' }
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

export type ActorLocalSimulationResult<State> = {
  provenance: ActorLocalSimulationProvenance;
  status: 'Completed' | 'Failed' | 'Suspended' | 'Closed';
  closeReason: 'RetryAttemptsExhausted' | 'ProductiveCycleCompleted' | null;
  cycleNonce: bigint;
  startCursor: number;
  runCursor: number | null;
  unsuccessfulAttemptsAtCursor: number | null;
  state: State;
  cumulative: ActorLocalSimulationCounts;
  journal: ActorLocalSimulationJournalEntry[];
};

export type ActorDonationSurface = {
  stepIndex: number;
  surface: string;
  resolution:
    | 'Fixed'
    | 'AllAvailable'
    | 'PercentageOfCurrent'
    | 'PercentageAtOpening'
    | 'PercentageOfLastFunding';
  observation: 'ActorBalance' | 'ActorFunding' | 'AdapterState';
};

export type ActorDonationSensitivity = {
  stepIndex: number;
  surface: string;
  sensitivity:
    | 'InsensitiveFixedAmount'
    | 'BeforeStepResolution'
    | 'BeforeOpeningSnapshot'
    | 'BeforeFundingSnapshot'
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

export function simulateActorLocally<State, Predicate>(input: {
  artifact: ActorContractArtifact;
  blockHash: ActorContractHex;
  model: string;
  modelVersion: string;
  cycleNonce: bigint;
  startCursor: number;
  completionPolicy?: 'Persistent' | 'CloseAfterProductiveCycle';
  unsuccessfulAttemptsAtCursor?: number;
  initialState: State;
  initialCounts?: ActorLocalSimulationCounts;
  steps: ActorLocalStep<Predicate>[];
  evaluatePredicate?: (
    predicate: ActorLocalTimedPredicate<Predicate>,
    state: Readonly<State>,
  ) => ActorLocalPredicateOutcome;
  runTask: (
    step: ActorLocalStep<Predicate>,
    taskLocalState: State,
  ) => ActorLocalStepOutcome;
}): ActorLocalSimulationResult<State> {
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

  for (let index = input.startCursor; index < input.steps.length; index += 1) {
    const step = input.steps[index];
    const taskLocalState = structuredClone(state);
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
        if (!expressionTruth) return { kind: 'SkippedPrecondition' } as const;
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
      const productiveClose =
        input.completionPolicy === 'CloseAfterProductiveCycle' &&
        cumulative.committedEffectfulTasks > 0;
      return {
        provenance: provenance(input),
        status: productiveClose ? 'Closed' : 'Completed',
        closeReason: productiveClose ? 'ProductiveCycleCompleted' : null,
        cycleNonce: input.cycleNonce,
        startCursor: input.startCursor,
        runCursor: null,
        unsuccessfulAttemptsAtCursor: null,
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
        startCursor: input.startCursor,
        runCursor: exhausted ? null : index,
        unsuccessfulAttemptsAtCursor: exhausted
          ? null
          : nextUnsuccessfulAttempts,
        state,
        cumulative,
        journal,
      };
    }
    if (outcome.kind === 'Failed' && step.onError.type !== 'ContinueNextStep') {
      return {
        provenance: provenance(input),
        status: 'Failed',
        closeReason: null,
        cycleNonce: input.cycleNonce,
        startCursor: input.startCursor,
        runCursor: null,
        unsuccessfulAttemptsAtCursor: null,
        state,
        cumulative,
        journal,
      };
    }
  }

  const productiveClose =
    input.completionPolicy === 'CloseAfterProductiveCycle' &&
    cumulative.committedEffectfulTasks > 0;
  return {
    provenance: provenance(input),
    status: productiveClose ? 'Closed' : 'Completed',
    closeReason: productiveClose ? 'ProductiveCycleCompleted' : null,
    cycleNonce: input.cycleNonce,
    startCursor: input.startCursor,
    runCursor: null,
    unsuccessfulAttemptsAtCursor: null,
    state,
    cumulative,
    journal,
  };
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
    if (surface.resolution === 'PercentageAtOpening') {
      return {
        stepIndex: surface.stepIndex,
        surface: surface.surface,
        sensitivity: 'BeforeOpeningSnapshot',
        reason:
          'Actor balance changes can affect the captured opening snapshot, but not its persisted value.',
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
