/*
Domain: Governance view contracts
Owns: Store/view-facing governance proposal descriptors and viewer state shapes.
Excludes: Runtime payload primitives, adapter transport, widget rendering, and mutation lifecycle.
Zone: Governance presentation contract; safe for stores, labels, and widgets to import.
*/
import type { ReadModelValue } from '$lib/read-model';

import type {
  GovernanceAccountId,
  GovernanceAccountPowerView,
  GovernanceAuthorizedRuntimeUpgrade,
  GovernanceGovXpCounters,
  GovernancePrimaryTrackOption,
  GovernanceProposalExecutionAuthority,
  GovernanceProposalMetadata,
  GovernanceProposalOpeningFee,
  GovernanceProposalPayloadAvailability,
  GovernanceProposalPayloadKind,
  GovernanceProposalPrimaryTrackFamily,
  GovernanceProposalPrimaryTrackTally,
  GovernanceProposalStatus,
  GovernanceProposalSubmissionAuthority,
  GovernanceProposalTiming,
  GovernanceProposalVoteTally,
  GovernanceProviderState,
  GovernanceRecentFinalizedProposal,
  GovernanceVoteKind,
  GovernanceVotePowerProfile,
  GovernanceWriteSurfaceAvailability,
} from './types';

export type GovernanceProposalDescriptor = {
  metadata: GovernanceProposalMetadata | null;
  executionAuthority: GovernanceProposalExecutionAuthority | null;
  submissionAuthority: GovernanceProposalSubmissionAuthority | null;
  openingFee: GovernanceProposalOpeningFee | null;
  payloadAvailability: GovernanceProposalPayloadAvailability | null;
  primaryTrackFamily: GovernanceProposalPrimaryTrackFamily | null;
  urgentEligibility: boolean | null;
};

export type GovernancePanelProposal = GovernanceProposalDescriptor & {
  itemId: number;
  status: GovernanceProposalStatus | null;
  timing: GovernanceProposalTiming | null;
  primaryTrackTally: GovernanceProposalPrimaryTrackTally | null;
  tally: GovernanceProposalVoteTally | null;
  accountPowerView: GovernanceAccountPowerView | null;
  votePowerProfiles: Partial<
    Record<GovernanceVoteKind, GovernanceVotePowerProfile>
  >;
};

export type GovernanceRetainedFinalizedProposal =
  GovernanceRecentFinalizedProposal &
    GovernanceProposalDescriptor & {
      winningPrimaryOption: GovernancePrimaryTrackOption | null;
    };

export type GovernancePublicSubmissionOption = {
  payloadKind: GovernanceProposalPayloadKind;
  openingFee: GovernanceProposalOpeningFee | null;
};

export type GovernanceViewerState = {
  providerState: GovernanceProviderState;
  endpoint: string;
  domainId: number;
  accountId: GovernanceAccountId;
  activeProposalIds: number[];
  activeProposals: GovernancePanelProposal[];
  submissionOptions: GovernancePublicSubmissionOption[];
  authorizedRuntimeUpgrade: GovernanceAuthorizedRuntimeUpgrade | null;
  recentFinalizedProposals: GovernanceRetainedFinalizedProposal[];
  recentFinalizedProposalsView: ReadModelValue<
    GovernanceRetainedFinalizedProposal[]
  > | null;
  rewardCoefficient: string | null;
  govxpCounters: GovernanceGovXpCounters;
  loading: boolean;
  error: string | null;
  writeError: string | null;
  writeSurfaceAvailability: GovernanceWriteSurfaceAvailability;
};
