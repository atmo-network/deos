/*
Domain: AAA eligibility transport
Owns: Read-only runtime `AaaEligibilityApi` invocation at one finalized block and canonical projection.
Excludes: Scheduler semantics, execution timing, storage topology, and plan authoring.
Zone: Blockchain adapter capability; imports the automation eligibility contract only.
*/
import type { DeosTypedApi } from './deos.ts';

import {
  type AaaEligibilityProjection,
  projectAaaEligibility,
} from '../../automation/eligibility.ts';

export type AaaEligibilityRead = {
  projection: AaaEligibilityProjection | null;
  unavailableReason: string | null;
};

type AaaEligibilityAt = NonNullable<
  Parameters<DeosTypedApi['apis']['AaaEligibilityApi']['aaa_eligibility']>[1]
>['at'];

export async function readAaaEligibility(
  typedApi: DeosTypedApi,
  at: AaaEligibilityAt,
  aaaId: number,
): Promise<AaaEligibilityRead> {
  try {
    const result = await typedApi.apis.AaaEligibilityApi.aaa_eligibility(
      BigInt(aaaId),
      { at },
    );
    return {
      projection: projectAaaEligibility(result),
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
