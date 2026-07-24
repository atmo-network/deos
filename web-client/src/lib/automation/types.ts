/*
Domain: Automation contracts
Owns: System AAA actor snapshots plus portable plan-authoring policy shapes.
Excludes: Runtime actor scheduling, adapter transport, transaction composition, and widget rendering.
Zone: Automation public contract; safe for adapters, stores, and widgets to import.
*/
export const AUTOMATION_STEP_ERROR_POLICIES = [
  'AbortCycle',
  'ContinueNextStep',
  'RetryLater',
] as const;

export type AutomationStepErrorPolicy =
  (typeof AUTOMATION_STEP_ERROR_POLICIES)[number];
export type AutomationMutability = 'Mutable' | 'Immutable';
export type AutomationRunState = 'idle' | 'suspended';

export type AutomationContinuationSnapshot = {
  cursor: number;
  attempt: number;
  lastAttemptBlock: number;
};

export function automationPolicyAllowed(
  mutability: AutomationMutability,
  policy: AutomationStepErrorPolicy,
): boolean {
  return mutability === 'Mutable' || policy !== 'RetryLater';
}

export type AutomationActorSnapshot = {
  aaaId: number;
  label: string;
  role: string;
  exists: boolean;
  paused: boolean;
  runState: AutomationRunState;
  cycleNonce: bigint;
  continuation: AutomationContinuationSnapshot | null;
  lastCycleBlock: number | null;
  triggerLabel: string;
  nativeBalance: bigint;
};
