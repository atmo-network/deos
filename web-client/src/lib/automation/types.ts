/*
Domain: Automation contracts
Owns: System Actor snapshots plus portable plan-authoring policy shapes.
Excludes: Runtime actor scheduling, adapter transport, transaction composition, and widget rendering.
Zone: Automation public contract; safe for adapters, stores, and widgets to import.
*/
import type { ActorEligibilityProjection } from './eligibility.ts';
import type {
  ActorPlanHex,
  ActorPlanRuntimeIdentity,
} from './plan-artifact.ts';

export const AUTOMATION_STEP_ERROR_POLICIES = [
  'AbortCycle',
  'ContinueNextStep',
  'RetryLater',
] as const;

export type AutomationStepErrorPolicy =
  | { type: 'AbortCycle' }
  | { type: 'ContinueNextStep' }
  | { type: 'RetryLater'; maxAttempts: number };
export type AutomationMutability = 'Mutable' | 'Immutable';
export type AutomationRunState = 'idle' | 'suspended';

export type AutomationContinuationSnapshot = {
  cursor: number;
  attempt: number;
  unsuccessfulAttemptsAtCursor: number;
  lastAttemptBlock: number;
};

export function automationPolicyAllowed(
  mutability: AutomationMutability,
  policy: AutomationStepErrorPolicy,
): boolean {
  return mutability === 'Mutable' || policy.type !== 'RetryLater';
}

export type AutomationAuthoringContext = {
  metadataBytes: Uint8Array;
  runtime: ActorPlanRuntimeIdentity;
  finalizedBlock: {
    hash: ActorPlanHex;
    number: number;
  };
};

export type AutomationActorSnapshot = {
  actorId: number;
  label: string;
  role: string;
  exists: boolean;
  /** Actor class from the identity locator: 'System' or 'User'. */
  actorClass: 'System' | 'User' | null;
  paused: boolean;
  runState: AutomationRunState;
  cycleNonce: bigint;
  continuation: AutomationContinuationSnapshot | null;
  lastCycleBlock: number | null;
  completionPolicy: 'Persistent' | 'CloseAfterProductiveCycle' | null;
  triggerLabel: string;
  nativeBalance: bigint;
  /** One-FIFO membership: the live queue ticket, or null when the actor is not queued. */
  queueTicket: bigint | null;
  /** Bounded per-asset funding accumulator awaiting the next cycle open. */
  fundingAccumulated: ReadonlyArray<[string, bigint]>;
  /** Funding source policy as a typed variant label. */
  fundingSourcePolicy: string | null;
  /**
   * Scheduler-owned eligibility projection from the read-only runtime API
   * (`phase`, optional close reason, and `nextEligibleBlock`), or null when the runtime API
   * is unavailable. Clients never reimplement cadence, cooldown, window, retry
   * backoff, breaker, or latch arithmetic.
   */
  eligibility: ActorEligibilityProjection | null;
};
