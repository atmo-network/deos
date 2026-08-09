/*
Domain: Actors eligibility transport
Owns: Read-only runtime `ActorEligibilityApi` invocation at one finalized block and canonical projection.
Excludes: Scheduler semantics, execution timing, storage topology, and plan authoring.
Zone: Blockchain adapter capability; imports the automation eligibility contract only.
*/
import type { DeosTypedApi } from './deos.ts';

import {
  type ActorEligibilityProjection,
  projectActorEligibility,
} from '../../automation/eligibility.ts';

export type ActorEligibilityRead = {
  projection: ActorEligibilityProjection | null;
  unavailableReason: string | null;
};

type ActorEligibilityAt = NonNullable<
  Parameters<
    DeosTypedApi['apis']['ActorEligibilityApi']['actor_eligibility']
  >[1]
>['at'];

export async function readActorEligibility(
  typedApi: DeosTypedApi,
  at: ActorEligibilityAt,
  actorId: number,
): Promise<ActorEligibilityRead> {
  try {
    const result = await typedApi.apis.ActorEligibilityApi.actor_eligibility(
      BigInt(actorId),
      { at },
    );
    return {
      projection: projectActorEligibility(result),
      unavailableReason: null,
    };
  } catch (error) {
    return {
      projection: null,
      unavailableReason:
        error instanceof Error ? error.message : 'Eligibility API unavailable',
    };
  }
}
