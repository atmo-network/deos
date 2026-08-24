/*
Domain: Actors block-resource projection
Owns: Named browser contracts for configured, current, and finalized canonical block-resource facts.
Excludes: Runtime transport, scheduler reconstruction, archive history, and admission decisions.
Zone: Automation domain; transport adapters lower finalized runtime API values into these contracts.
*/

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
  finalizedFixedReserved: ActorResourceWeight | null;
  optionalActorWorkHalted: boolean;
};

export type FinalizedActorResourceView = {
  blockNumber: number;
  fixedReserved: ActorResourceWeight;
  usage: ActorResourceUsageView;
  optionalActorWorkHalted: boolean;
};

export type ActorResourceProjection = {
  budget: ActorResourceBudgetView;
  current: CurrentActorResourceView | null;
  finalized: FinalizedActorResourceView | null;
};
