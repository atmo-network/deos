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
import { projectActorMachineControlFlags } from '../../automation/resource.ts';

type ActorResourceAt = NonNullable<
  Parameters<
    DeosTypedApi['apis']['ActorResourceApi']['block_resource_budget']
  >[0]
>['at'];

type RuntimeBudget = Awaited<
  ReturnType<DeosTypedApi['apis']['ActorResourceApi']['block_resource_budget']>
>;
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

function projectBudget(value: RuntimeBudget): ActorResourceBudgetView {
  return {
    maximumBlock: weight(value.maximum_block),
    fixedEnvelope: weight(value.fixed_envelope),
    limits: {
      actorControl: weight(value.limits.actor_control),
      sharedEconomic: weight(value.limits.shared_economic),
      actorBaseTurn: weight(value.limits.actor_base_turn),
      userBaseTurn: weight(value.limits.user_base_turn),
    },
  };
}

export async function readActorResourceProjection(
  typedApi: DeosTypedApi,
  at: ActorResourceAt,
): Promise<ActorResourceProjection> {
  const [runtimeBudget, runtimeCurrent, runtimeFinalized, machineControl] =
    await Promise.all([
      typedApi.apis.ActorResourceApi.block_resource_budget({ at }),
      typedApi.apis.ActorResourceApi.current_block_resource_state({ at }),
      typedApi.apis.ActorResourceApi.finalized_block_resource_snapshot({ at }),
      readMachineControlFlags(typedApi, at),
    ]);

  const budget = projectBudget(runtimeBudget);
  const current: CurrentActorResourceView | null = runtimeCurrent
    ? {
        blockNumber: runtimeCurrent.block_number,
        phase: runtimeCurrent.phase.type,
        usage: usage(runtimeCurrent.usage),
        outstandingReservations: runtimeCurrent.outstanding_reservations,
        budget: runtimeCurrent.budget
          ? projectBudget(runtimeCurrent.budget)
          : null,
        optionalActorWorkHalted: runtimeCurrent.optional_actor_work_halted,
      }
    : null;
  const finalized: FinalizedActorResourceView | null = runtimeFinalized
    ? {
        blockNumber: runtimeFinalized.block_number,
        budget: projectBudget(runtimeFinalized.budget),
        usage: usage(runtimeFinalized.usage),
        optionalActorWorkHalted: runtimeFinalized.optional_actor_work_halted,
      }
    : null;

  return { budget, current, finalized, ...machineControl };
}

async function readMachineControlFlags(
  typedApi: unknown,
  at: ActorResourceAt,
): Promise<
  Pick<
    ActorResourceProjection,
    'machineControl' | 'machineControlUnavailableReason'
  >
> {
  try {
    const apis = (
      typedApi as {
        apis?: { ActorResourceApi?: { machine_control_flags?: unknown } };
      }
    ).apis;
    const owner = apis?.ActorResourceApi;
    const method = owner?.machine_control_flags;
    if (typeof method !== 'function')
      throw new Error('Machine control metadata is unavailable');
    return {
      machineControl: projectActorMachineControlFlags(
        await method.call(owner, { at }),
      ),
      machineControlUnavailableReason: null,
    };
  } catch (error) {
    return {
      machineControl: null,
      machineControlUnavailableReason:
        error instanceof Error
          ? error.message
          : 'Machine control flags unavailable',
    };
  }
}
