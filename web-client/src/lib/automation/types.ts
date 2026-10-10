/*
Domain: Automation contracts
Owns: System Actor snapshots plus portable plan-authoring policy shapes.
Excludes: Runtime actor scheduling, adapter transport, transaction composition, and widget rendering.
Zone: Automation public contract; safe for adapters, stores, and widgets to import.
*/
import type { ActorOnUnfunded } from './behavior-policy.ts';
import type {
  ActorContractHex,
  ActorContractRuntimeIdentity,
} from './contract-artifact.ts';
import type { ActorEligibilityView } from './eligibility.ts';

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
  runtime: ActorContractRuntimeIdentity;
  finalizedBlock: {
    hash: ActorContractHex;
    number: number;
  };
};

export type AutomationActorResidence =
  | { type: 'Service'; kind: 'Live' | 'Pending' }
  | {
      type: 'Deadline';
      key: { type: 'Block'; value: number } | { type: 'Tick'; value: bigint };
    }
  | { type: 'Parked'; reason: string }
  | { type: 'Disabled' };

export function automationResidenceLabel(
  residence: AutomationActorResidence | null,
): string {
  if (residence === null) return 'Unavailable';
  switch (residence.type) {
    case 'Service':
      return `Service · ${residence.kind}`;
    case 'Deadline':
      return `${residence.key.type === 'Tick' ? 'Tick' : 'Block'} ${residence.key.value}`;
    case 'Parked':
      return `Parked · ${residence.reason}`;
    case 'Disabled':
      return 'Disabled';
  }
}

export type AutomationActorSnapshot = {
  actorId: number;
  label: string;
  role: string;
  exists: boolean;
  /** Actor class from the canonical semantic identity: 'System' or 'User'. */
  actorClass: 'System' | 'User' | null;
  paused: boolean;
  runState: AutomationRunState;
  cycleNonce: bigint;
  continuation: AutomationContinuationSnapshot | null;
  lastCycleBlock: number | null;
  /** Header presence only; the cold expression and monitor are separate owners. */
  hasCloseWhen: boolean | null;
  onUnfunded: ActorOnUnfunded | null;
  triggerLabel: string;
  nativeBalance: bigint;
  controlStatus: 'NotRegistered' | 'Dormant' | 'Active' | 'Unavailable';
  residence: AutomationActorResidence | null;
  controlUnavailableReason: string | null;
  awaitingFunding: boolean;
  /** Funding source policy as a typed variant label. */
  fundingSourcePolicy: string | null;
  /**
   * Scheduler-owned eligibility projection from the read-only runtime API
   * (absence, dormancy, or canonical Active classification), or null when the runtime API
   * is unavailable. Clients never reimplement cadence, cooldown, window, retry
   * backoff, breaker, or latch arithmetic.
   */
  eligibility: ActorEligibilityView | null;
};

export function projectActorOutcomeHeader(header: unknown): {
  hasCloseWhen: boolean;
  onUnfunded: ActorOnUnfunded;
} {
  if (header === null || typeof header !== 'object' || Array.isArray(header)) {
    throw new Error('Actor outcome header is unavailable');
  }
  const record = header as Record<string, unknown>;
  const funding = record.on_unfunded;
  if (
    typeof record.has_close_when !== 'boolean' ||
    funding === null ||
    typeof funding !== 'object' ||
    Array.isArray(funding)
  ) {
    throw new Error('Actor outcome header requires current metadata');
  }
  const variant = funding as Record<string, unknown>;
  if (
    (variant.type !== 'Close' && variant.type !== 'AwaitFunding') ||
    variant.value !== undefined
  ) {
    throw new Error('Unsupported Actor fee-shortfall policy');
  }
  return { hasCloseWhen: record.has_close_when, onUnfunded: variant.type };
}
