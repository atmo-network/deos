import type {
  GovernanceQuerySurfaceAvailability,
  GovernanceWriteSurfaceAvailability,
} from "./types";

export const DEFAULT_GOVERNANCE_WS_ENDPOINT = "ws://127.0.0.1:9988";
export const DEFAULT_GOVERNANCE_RPC_ENDPOINT = DEFAULT_GOVERNANCE_WS_ENDPOINT;

export const GOVERNANCE_QUERY_SURFACE_AVAILABILITY: GovernanceQuerySurfaceAvailability =
  {
    activeProposalDiscovery: "onchain",
    recentFinalizedDiscovery: "onchain",
    proposalStatus: "onchain",
    proposalMetadata: "onchain",
    proposalExecutionAuthority: "onchain",
    authorizedRuntimeUpgrade: "onchain",
    proposalSubmissionAuthority: "onchain",
    proposalOpeningFee: "onchain",
    proposalPayloadAvailability: "onchain",
    payloadHashPreimageStatus: "onchain",
    payloadPreimageNoteCost: "onchain",
    proposalPrimaryTrackFamily: "onchain",
    proposalPrimaryTrackTally: "onchain",
    proposalWinningPrimaryOption: "onchain",
    proposalTiming: "onchain",
    proposalUrgentEligibility: "onchain",
    proposalTally: "onchain",
    votePowerProfiles: "onchain",
    rewardCoefficient: "onchain",
    govxpCounters: "onchain",
    proposalExecutionDetail: "onchain",
    ballotTimelines: "materialized",
    archiveSearch: "materialized",
  };

export const GOVERNANCE_RUNTIME_WRITE_SURFACE: GovernanceWriteSurfaceAvailability =
  {
    castVote: {
      runtimeAccess: "signed",
      providerStatus: "unavailable",
      reason: "Provider support not declared yet",
    },
    submitProposal: {
      runtimeAccess: "signed",
      providerStatus: "unavailable",
      reason: "Provider support not declared yet",
    },
    noteProposalPreimage: {
      runtimeAccess: "signed",
      providerStatus: "unavailable",
      reason: "Provider support not declared yet",
    },
    resolveProposal: {
      runtimeAccess: "admin",
      providerStatus: "unavailable",
      reason: "Provider support not declared yet",
    },
    rejectProposal: {
      runtimeAccess: "admin",
      providerStatus: "unavailable",
      reason: "Provider support not declared yet",
    },
    resolveProposalFromVotes: {
      runtimeAccess: "admin",
      providerStatus: "unavailable",
      reason: "Provider support not declared yet",
    },
    forceResolveProposalFromVotes: {
      runtimeAccess: "admin",
      providerStatus: "unavailable",
      reason: "Provider support not declared yet",
    },
    requeueProposalForAutoFinalization: {
      runtimeAccess: "admin",
      providerStatus: "unavailable",
      reason: "Provider support not declared yet",
    },
  };
