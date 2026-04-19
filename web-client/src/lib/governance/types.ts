import type {
  GovernanceAccountId,
  GovernanceAuthorizedRuntimeUpgrade,
  GovernanceProposalExecutionAuthority,
  GovernanceProposalOpeningFee,
  GovernanceProposalPayloadKind,
  GovernanceProposalSubmissionAuthority,
  GovernancePrimaryTrackOption,
  GovernanceProposalExecutionDetail,
  GovernanceProposalMetadata,
  GovernanceProposalPayloadAvailability,
  GovernanceProposalPrimaryTrackFamily,
  GovernanceProposalPrimaryTrackTally,
  GovernanceProposalStatus,
  GovernanceProposalTiming,
  GovernanceProposalVoteTally,
  GovernanceProviderState,
  GovernanceRecentFinalizedProposal,
  GovernanceVoteKind,
  GovernanceVotePowerProfile,
  GovernanceWriteSurfaceAvailability,
} from "$lib/adapters/governance";
import type { ReadModelValue } from "$lib/shared/read-model";

export type GovernancePanelProposal = {
  itemId: number;
  status: GovernanceProposalStatus | null;
  metadata: GovernanceProposalMetadata | null;
  executionAuthority: GovernanceProposalExecutionAuthority | null;
  submissionAuthority: GovernanceProposalSubmissionAuthority | null;
  openingFee: GovernanceProposalOpeningFee | null;
  payloadAvailability: GovernanceProposalPayloadAvailability | null;
  primaryTrackFamily: GovernanceProposalPrimaryTrackFamily | null;
  timing: GovernanceProposalTiming | null;
  urgentEligibility: boolean | null;
  primaryTrackTally: GovernanceProposalPrimaryTrackTally | null;
  tally: GovernanceProposalVoteTally | null;
  votePowerProfiles: Partial<Record<GovernanceVoteKind, GovernanceVotePowerProfile>>;
};

export type GovernanceRetainedFinalizedProposal = GovernanceRecentFinalizedProposal & {
  metadata: GovernanceProposalMetadata | null;
  executionAuthority: GovernanceProposalExecutionAuthority | null;
  submissionAuthority: GovernanceProposalSubmissionAuthority | null;
  openingFee: GovernanceProposalOpeningFee | null;
  payloadAvailability: GovernanceProposalPayloadAvailability | null;
  primaryTrackFamily: GovernanceProposalPrimaryTrackFamily | null;
  winningPrimaryOption: GovernancePrimaryTrackOption | null;
  urgentEligibility: boolean | null;
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
  recentFinalizedProposalsView: ReadModelValue<GovernanceRetainedFinalizedProposal[]> | null;
  rewardCoefficient: string | null;
  govxpCounters: {
    rollingWinningParticipation: number;
    totalParticipations: bigint;
    totalWinningParticipations: bigint;
    totalAuthoredProposals: bigint;
    totalSuccessfulAuthoredProposals: bigint;
  };
  loading: boolean;
  error: string | null;
  writeError: string | null;
  writeSurfaceAvailability: GovernanceWriteSurfaceAvailability;
};
