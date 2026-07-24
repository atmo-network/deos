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
  | 'AbortCycle'
  | 'ContinueNextStep'
  | 'RetryLater';

export type AaaLocalStep = {
  stepIndex: number;
  onError: AaaStepErrorPolicy;
};

export type AaaLocalStepOutcome =
  | { kind: 'Executed' }
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
  status: 'Completed' | 'Aborted' | 'Suspended';
  cycleNonce: bigint;
  attempt: number;
  startCursor: number;
  continuationCursor: number | null;
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

export function simulateAaaLocally<State>(input: {
  artifact: AaaPlanArtifact;
  blockHash: AaaPlanHex;
  model: string;
  modelVersion: string;
  cycleNonce: bigint;
  attempt: number;
  startCursor: number;
  initialState: State;
  initialCounts?: AaaLocalSimulationCounts;
  steps: AaaLocalStep[];
  runStep: (step: AaaLocalStep, taskLocalState: State) => AaaLocalStepOutcome;
}): AaaLocalSimulationResult<State> {
  validateIndex(input.attempt, 'attempt');
  validateIndex(input.startCursor, 'startCursor');
  if (input.cycleNonce < 0n) throw new Error('cycleNonce must be non-negative');
  if (input.startCursor > input.steps.length) {
    throw new Error('startCursor exceeds the plan length');
  }
  input.steps.forEach((step, index) => {
    if (step.stepIndex !== index) {
      throw new Error('steps must use contiguous ordered indices');
    }
    if (
      !['AbortCycle', 'ContinueNextStep', 'RetryLater'].includes(step.onError)
    ) {
      throw new Error(`Unsupported error policy at step ${index}`);
    }
    if (
      input.artifact.mutability === 'Immutable' &&
      step.onError === 'RetryLater'
    ) {
      throw new Error('RetryLater remains Mutable-only');
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
    const outcome = input.runStep(step, taskLocalState);
    let stateCommitted = false;
    switch (outcome.kind) {
      case 'Executed':
        state = taskLocalState;
        stateCommitted = true;
        increment(cumulative, 'executedSteps');
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

    if (outcome.kind === 'Failed') {
      if (step.onError === 'RetryLater' && outcome.retry === 'Temporary') {
        return {
          provenance: provenance(input),
          status: 'Suspended',
          cycleNonce: input.cycleNonce,
          attempt: input.attempt,
          startCursor: input.startCursor,
          continuationCursor: index,
          finalizedThrough: index === 0 ? null : index - 1,
          state,
          cumulative,
          journal,
        };
      }
      if (step.onError !== 'ContinueNextStep') {
        return {
          provenance: provenance(input),
          status: 'Aborted',
          cycleNonce: input.cycleNonce,
          attempt: input.attempt,
          startCursor: input.startCursor,
          continuationCursor: null,
          finalizedThrough: index,
          state,
          cumulative,
          journal,
        };
      }
    }
    finalizedThrough = index;
  }

  return {
    provenance: provenance(input),
    status: 'Completed',
    cycleNonce: input.cycleNonce,
    attempt: input.attempt,
    startCursor: input.startCursor,
    continuationCursor: null,
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
