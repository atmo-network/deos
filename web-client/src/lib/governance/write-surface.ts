import { GOVERNANCE_RUNTIME_WRITE_SURFACE } from "./constants";
import type {
  GovernanceWriteCapability,
  GovernanceWriteSurfaceAvailability,
} from "./types";

export function buildWriteSurfaceAvailability(
  overrides: Partial<
    Record<
      keyof GovernanceWriteSurfaceAvailability,
      Partial<GovernanceWriteCapability>
    >
  >,
): GovernanceWriteSurfaceAvailability {
  const result = {
    ...GOVERNANCE_RUNTIME_WRITE_SURFACE,
  } as GovernanceWriteSurfaceAvailability;
  for (const key of Object.keys(overrides) as Array<
    keyof GovernanceWriteSurfaceAvailability
  >) {
    const value = overrides[key];
    if (!value) {
      continue;
    }
    result[key] = { ...result[key], ...value };
  }
  return result;
}
