/*
Domain: Actors materialization projection
Owns: Named browser contracts for canonical Crossing capacity and current materialization faults.
Excludes: Runtime transport, fault repair, scheduler reconstruction, and historical indexing.
Zone: Automation domain; finalized adapters lower runtime API values into these contracts.
*/

export type CrossingCapacityView = {
  userLimit: number;
  totalLimit: number;
  userMemberships: number;
  totalMemberships: number;
};

export type MaterializationFaultClass =
  | 'Invariant'
  | 'Capacity'
  | 'SchedulerExhausted'
  | 'Other';

export type CrossingMaterializationFault = {
  feed: unknown;
  revision: bigint | null;
  threshold: bigint | null;
  class: MaterializationFaultClass;
};

export type FanoutMaterializationFault = {
  feed: unknown;
  revision: bigint;
  subscriberPage: number | null;
  subscriberPosition: number;
  actorId: bigint | null;
  semanticContractId: Uint8Array | null;
  bodyCommitment: Uint8Array | null;
  admissionIdentity: Uint8Array | null;
  branch: string;
  class: MaterializationFaultClass;
};

export type WakeupMaterializationFault = {
  key: { type: 'Block'; block: number } | { type: 'Tick'; tick: bigint };
  page: number;
  class: MaterializationFaultClass;
};

export type MaterializationFaultsView = {
  crossing: CrossingMaterializationFault | null;
  fanout: FanoutMaterializationFault | null;
  wakeup: WakeupMaterializationFault | null;
};

export type ActorMaterializationProjection = {
  crossingCapacity: CrossingCapacityView;
  faults: MaterializationFaultsView;
};
