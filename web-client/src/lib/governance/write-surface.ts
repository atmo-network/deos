/*
Domain: Governance write surface
Owns: Capability lookup helpers for runtime governance write availability.
Excludes: Transaction submission, adapter RPC implementation, and proposal draft state.
Zone: Governance helper; derives UI/write affordances from governance constants and contracts.
*/
import { GOVERNANCE_RUNTIME_WRITE_SURFACE } from './constants';
import type {
  GovernanceWriteCapability,
  GovernanceWriteOperation,
  GovernanceWriteSurfaceAvailability,
} from './types';

const GOVERNANCE_WRITE_OPERATIONS: GovernanceWriteOperation[] = [
  'castVote',
  'submitProposal',
  'noteProposalPreimage',
  'resolveProposal',
  'rejectProposal',
  'resolveProposalFromVotes',
  'forceResolveProposalFromVotes',
  'requeueProposalForAutoFinalization',
];

export function buildWriteSurfaceAvailability(
  overrides: Partial<
    Record<GovernanceWriteOperation, Partial<GovernanceWriteCapability>>
  >,
): GovernanceWriteSurfaceAvailability {
  const result: GovernanceWriteSurfaceAvailability = {
    ...GOVERNANCE_RUNTIME_WRITE_SURFACE,
  };
  for (const key of GOVERNANCE_WRITE_OPERATIONS) {
    const value = overrides[key];
    if (!value) {
      continue;
    }
    result[key] = { ...result[key], ...value };
  }
  return result;
}
