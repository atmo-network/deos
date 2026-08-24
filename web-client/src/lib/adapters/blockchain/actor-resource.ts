/*
Domain: Actors block-resource transport
Owns: Finalized ActorResourceApi reads and lossless lowering into automation resource contracts.
Excludes: Scheduler policy, resource arithmetic, archive history, and UI presentation.
Zone: Blockchain adapter capability; imports only the automation resource contract.
*/
import type { DeosTypedApi } from './deos.ts';

import type {
  ActorResourceBudgetView,
  ActorResourceProjection,
  ActorResourceUsageView,
  ActorResourceWeight,
  CurrentActorResourceView,
  FinalizedActorResourceView,
} from '../../automation/resource.ts';

type ActorResourceAt = NonNullable<
  Parameters<
    DeosTypedApi['apis']['ActorResourceApi']['block_resource_budget']
  >[0]
>['at'];

type RuntimeWeight = { ref_time: bigint; proof_size: bigint };
type RuntimeUsage = {
  actor_control: RuntimeWeight;
  actor_effect: RuntimeWeight;
  user_dispatch: RuntimeWeight;
};

function weight(value: RuntimeWeight): ActorResourceWeight {
  return { refTime: value.ref_time, proofSize: value.proof_size };
}

function usage(value: RuntimeUsage): ActorResourceUsageView {
  return {
    actorControl: weight(value.actor_control),
    actorEffect: weight(value.actor_effect),
    userDispatch: weight(value.user_dispatch),
  };
}

export async function readActorResourceProjection(
  typedApi: DeosTypedApi,
  at: ActorResourceAt,
): Promise<ActorResourceProjection> {
  const [runtimeBudget, runtimeCurrent, runtimeFinalized] = await Promise.all([
    typedApi.apis.ActorResourceApi.block_resource_budget({ at }),
    typedApi.apis.ActorResourceApi.current_block_resource_state({ at }),
    typedApi.apis.ActorResourceApi.finalized_block_resource_snapshot({ at }),
  ]);

  const budget: ActorResourceBudgetView = {
    maximumBlock: weight(runtimeBudget.maximum_block),
    fixedEnvelope: weight(runtimeBudget.fixed_envelope),
    limits: {
      actorControl: weight(runtimeBudget.limits.actor_control),
      sharedEconomic: weight(runtimeBudget.limits.shared_economic),
      actorBaseTurn: weight(runtimeBudget.limits.actor_base_turn),
      userBaseTurn: weight(runtimeBudget.limits.user_base_turn),
    },
  };
  const current: CurrentActorResourceView | null = runtimeCurrent
    ? {
        blockNumber: runtimeCurrent.block_number,
        phase: runtimeCurrent.phase.type,
        usage: usage(runtimeCurrent.usage),
        outstandingReservations: runtimeCurrent.outstanding_reservations,
        finalizedFixedReserved: runtimeCurrent.finalized_fixed_reserved
          ? weight(runtimeCurrent.finalized_fixed_reserved)
          : null,
        optionalActorWorkHalted: runtimeCurrent.optional_actor_work_halted,
      }
    : null;
  const finalized: FinalizedActorResourceView | null = runtimeFinalized
    ? {
        blockNumber: runtimeFinalized.block_number,
        fixedReserved: weight(runtimeFinalized.fixed_reserved),
        usage: usage(runtimeFinalized.usage),
        optionalActorWorkHalted: runtimeFinalized.optional_actor_work_halted,
      }
    : null;

  return { budget, current, finalized };
}
