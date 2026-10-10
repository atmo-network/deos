/*
Domain: Actors block-resource projection
Owns: Named browser contracts for configured, current, and finalized canonical block-resource facts.
Excludes: Runtime transport, scheduler reconstruction, archive history, and admission decisions.
Zone: Automation domain; transport adapters lower finalized runtime API values into these contracts.
*/

export const ACTORS_RESOURCE_RUNTIME_API_VERSION = 2 as const;

export type ActorMachineControlFlags = {
  userCreationEnabled: boolean;
  machineFrozen: boolean;
};

export function projectActorMachineControlFlags(
  value: unknown,
): ActorMachineControlFlags {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error('Machine control flags must be a runtime object');
  }
  const fields = value as Record<string, unknown>;
  if (
    typeof fields.user_creation_enabled !== 'boolean' ||
    typeof fields.machine_frozen !== 'boolean'
  ) {
    throw new Error(
      'Machine control flags must carry both independent booleans',
    );
  }
  return {
    userCreationEnabled: fields.user_creation_enabled,
    machineFrozen: fields.machine_frozen,
  };
}

export type ActorResourceWeight = {
  refTime: bigint;
  proofSize: bigint;
};

export type ActorResourceLimits = {
  actorControl: ActorResourceWeight;
  sharedEconomic: ActorResourceWeight;
  actorBaseTurn: ActorResourceWeight;
  userBaseTurn: ActorResourceWeight;
};

export type ActorResourceBudgetView = {
  maximumBlock: ActorResourceWeight;
  fixedEnvelope: ActorResourceWeight;
  limits: ActorResourceLimits;
};

export type ActorResourceUsageView = {
  actorControl: ActorResourceWeight;
  actorEffect: ActorResourceWeight;
  userDispatch: ActorResourceWeight;
};

export type ActorResourcePhase =
  | 'ContextIncomplete'
  | 'PrepassExecuting'
  | 'ExternalPhase'
  | 'FreshDrain'
  | 'Finalizable';

export type CurrentActorResourceView = {
  blockNumber: number;
  phase: ActorResourcePhase;
  usage: ActorResourceUsageView;
  outstandingReservations: number;
  budget: ActorResourceBudgetView | null;
  optionalActorWorkHalted: boolean;
};

export type FinalizedActorResourceView = {
  blockNumber: number;
  budget: ActorResourceBudgetView;
  usage: ActorResourceUsageView;
  optionalActorWorkHalted: boolean;
};

export type ActorResourceProjection = {
  machineControl: ActorMachineControlFlags | null;
  machineControlUnavailableReason: string | null;
  budget: ActorResourceBudgetView;
  current: CurrentActorResourceView | null;
  finalized: FinalizedActorResourceView | null;
};
