/*
Domain: Actors cost transport
Owns: Read-only ActorCostApi invocation at one finalized block and canonical cost projection.
Excludes: Fee estimation, storage assembly, event history, and presentation.
Zone: Blockchain adapter capability; imports only the automation cost contract.
*/
import type { DeosTypedApi } from './deos.ts';

import {
  type ActorCostView,
  projectActorCostQuote,
} from '../../automation/cost.ts';

export type ActorCostRead = {
  projection: ActorCostView | null;
  unavailableReason: string | null;
};

type ActorCostAt = NonNullable<
  Parameters<DeosTypedApi['apis']['ActorCostApi']['actor_cost_quote']>[1]
>['at'];

export async function readActorCost(
  typedApi: DeosTypedApi,
  at: ActorCostAt,
  actorId: number,
): Promise<ActorCostRead> {
  try {
    const result = await typedApi.apis.ActorCostApi.actor_cost_quote(
      BigInt(actorId),
      { at },
    );
    return {
      projection: projectActorCostQuote(result),
      unavailableReason: null,
    };
  } catch (error) {
    return {
      projection: null,
      unavailableReason:
        error instanceof Error ? error.message : 'Actor cost API unavailable',
    };
  }
}
