import type { TmctolChainConnectionState } from "../blockchain/deos";

export type GovernanceDomainId = number;
export type GovernanceItemId = number;
export type GovernanceEpoch = number;
export type GovernanceAccountId = string;
export type GovernanceWeight = bigint;
export type GovernanceRewardCoefficient = string;
export type GovernanceVoteKind =
  | "aye"
  | "nay"
  | "amplify"
  | "approve"
  | "reduce"
  | "veto"
  | "pass";
export type GovernanceProposalCadenceMode = "Ordinary" | "Fast";
export type GovernanceProposalPrimaryTrackFamily = "Binary" | "Invoice";
export type GovernancePrimaryTrackOption =
  | "Aye"
  | "Nay"
  | "Amplify"
  | "Approve"
  | "Reduce";
export type GovernanceVotePowerProfile =
  | "DecliningDirectStake"
  | "DecliningVetoAsset"
  | "DecliningNativeStake"
  | "FlatUrgentDirectStake";
export type GovernanceQuerySurfaceKind = "onchain" | "materialized";
export type GovernanceProposalRejectionReason =
  | "AdminRejected"
  | "NoVotes"
  | "VoteTie"
  | "TurnoutBelowMinimum"
  | "ApprovalThresholdNotMet";
export type GovernanceVetoCancellationMode =
  | "ImmediateThreshold"
  | "TrackOutcome";
export type GovernanceProposalResolutionState =
  | {
      kind: "VotingWindowOpen";
      currentEpoch: GovernanceEpoch;
      maturityEpoch: GovernanceEpoch;
    }
  | {
      kind: "VetoPassing";
      vetoWeight: GovernanceWeight;
      passWeight: GovernanceWeight;
      mode: GovernanceVetoCancellationMode;
    }
  | { kind: "PassingAye" }
  | { kind: "PassingAmplify" }
  | { kind: "PassingApprove" }
  | { kind: "PassingReduce" }
  | { kind: "PassingNay" }
  | {
      kind: "Confirming";
      confirmStartedEpoch: GovernanceEpoch;
      confirmEndEpoch: GovernanceEpoch;
    }
  | {
      kind: "Rejected";
      reason: GovernanceProposalRejectionReason;
    };
export type GovernanceFinalizedProposalOutcome =
  | {
      kind: "Resolved";
      epoch: GovernanceEpoch;
      winnerCount: number;
    }
  | {
      kind: "Rejected";
      epoch: GovernanceEpoch;
      reason: GovernanceProposalRejectionReason;
    }
  | {
      kind: "VetoCancelled";
      epoch: GovernanceEpoch;
      vetoWeight: GovernanceWeight;
    }
  | {
      kind: "Enacted";
      approvedEpoch: GovernanceEpoch;
      executedEpoch: GovernanceEpoch;
      winnerCount: number;
    }
  | {
      kind: "ExecutionFailed";
      approvedEpoch: GovernanceEpoch;
      failedEpoch: GovernanceEpoch;
      winnerCount: number;
    }
  | {
      kind: "AdvisoryFinalized";
      approvedEpoch: GovernanceEpoch;
      finalizedEpoch: GovernanceEpoch;
      winnerCount: number;
    };
export type GovernanceProposalPayloadKind =
  | "L1RootAction"
  | "L2TreasurySpend"
  | "L2ParameterChange"
  | "Intent"
  | "L2SignalToL1";
export type GovernanceProposalMetadata = {
  cadenceMode: GovernanceProposalCadenceMode;
  payloadKind: GovernanceProposalPayloadKind;
  payloadHash: string;
};
export type GovernanceProposalPayloadAvailability = {
  havePreimage: boolean;
  preimageRequested: boolean;
};
export type GovernancePayloadHashPreimageStatus = {
  havePreimage: boolean;
  preimageRequested: boolean;
  byteLength: number | null;
};
export type GovernanceProposalTiming = {
  submittedEpoch: GovernanceEpoch;
  protectionOpenEpoch: GovernanceEpoch;
  protectionCloseEpoch: GovernanceEpoch;
  ordinaryPrimaryOpenEpoch: GovernanceEpoch;
  ordinaryPrimaryCloseEpoch: GovernanceEpoch;
  urgentPrimaryOpenEpoch: GovernanceEpoch | null;
  urgentPrimaryCloseEpoch: GovernanceEpoch | null;
  effectivePrimaryOpenEpoch: GovernanceEpoch;
  effectivePrimaryCloseEpoch: GovernanceEpoch;
  pendingEnactmentEpoch: GovernanceEpoch | null;
};
export type GovernanceProposalExecutionAuthority =
  | "Root"
  | "DomainTreasury"
  | "DomainParameters"
  | "NonExecutable";
export type GovernanceAuthorizedRuntimeUpgrade = {
  codeHash: string;
  checkVersion: boolean;
};
export type GovernanceProposalSubmissionAuthority = "Signed" | "AdminOnly";
export type GovernanceProposalOpeningFee = bigint;
export type GovernancePayloadPreimageNoteCost = bigint;
export type GovernanceProposalExecutionFailureReason =
  | "MissingPreimage"
  | "InvalidPreimage"
  | "UnsupportedDomain"
  | "UnsupportedCall"
  | "UnsupportedTarget"
  | "UnsupportedPayloadKind"
  | "MissingWinningPrimaryOption"
  | "DispatchFailed";
export type GovernanceProposalParameterChangeSurface =
  | "RouterFee"
  | "TrackedAsset";
export type GovernanceProposalTreasurySpendSettlementKind =
  | "DirectTransfer"
  | "InvoiceScalarTransfer";
export type GovernanceProposalTreasurySpendScalar =
  | "Amplify"
  | "Approve"
  | "Reduce";
export type GovernanceProposalExecutionSuccessDetail =
  | {
      kind: "Generic";
    }
  | {
      kind: "RuntimeUpgradeAuthorized";
      codeHash: string;
    }
  | {
      kind: "ParameterChangeExecuted";
      surface: GovernanceProposalParameterChangeSurface;
    }
  | {
      kind: "TreasurySpendExecuted";
      fundingSource: GovernanceAccountId;
      beneficiary: GovernanceAccountId;
      payoutAsset: GovernanceDomainId;
      baseAmount: bigint;
      scalar: GovernanceProposalTreasurySpendScalar;
      finalAmount: bigint;
      settlementKind: GovernanceProposalTreasurySpendSettlementKind;
    };
export type GovernanceProposalExecutionDetail =
  | {
      kind: "Executed";
      payloadKind: GovernanceProposalPayloadKind;
      authority: GovernanceProposalExecutionAuthority;
      executedEpoch: GovernanceEpoch;
      detail: GovernanceProposalExecutionSuccessDetail;
    }
  | {
      kind: "ExecutionFailed";
      payloadKind: GovernanceProposalPayloadKind;
      authority: GovernanceProposalExecutionAuthority;
      failedEpoch: GovernanceEpoch;
      reason: GovernanceProposalExecutionFailureReason;
    }
  | {
      kind: "AdvisoryFinalized";
      payloadKind: GovernanceProposalPayloadKind;
      finalizedEpoch: GovernanceEpoch;
    };
export type GovernanceProposalStatus =
  | {
      kind: "Active";
      resolution: GovernanceProposalResolutionState;
    }
  | {
      kind: "PendingEnactment";
      outcome: GovernanceFinalizedProposalOutcome;
      enactmentEpoch: GovernanceEpoch;
    }
  | {
      kind: "Finalized";
      outcome: GovernanceFinalizedProposalOutcome;
    };
export type GovernanceProposalVoteTally = {
  ayeVoters: number;
  nayVoters: number;
  amplifyVoters: number;
  approveVoters: number;
  reduceVoters: number;
  vetoVoters: number;
  passVoters: number;
  ayeWeight: GovernanceWeight;
  nayWeight: GovernanceWeight;
  amplifyWeight: GovernanceWeight;
  approveWeight: GovernanceWeight;
  reduceWeight: GovernanceWeight;
  vetoWeight: GovernanceWeight;
  passWeight: GovernanceWeight;
  turnoutWeight: GovernanceWeight;
  vetoTurnoutWeight: GovernanceWeight;
};
export type GovernanceProposalPrimaryTrackTally =
  | {
      kind: "Binary";
      ayeVoters: number;
      nayVoters: number;
      ayeWeight: GovernanceWeight;
      nayWeight: GovernanceWeight;
      turnoutWeight: GovernanceWeight;
      leadingOption: GovernancePrimaryTrackOption | null;
    }
  | {
      kind: "Invoice";
      amplifyVoters: number;
      approveVoters: number;
      reduceVoters: number;
      nayVoters: number;
      amplifyWeight: GovernanceWeight;
      approveWeight: GovernanceWeight;
      reduceWeight: GovernanceWeight;
      nayWeight: GovernanceWeight;
      positiveWeight: GovernanceWeight;
      turnoutWeight: GovernanceWeight;
      leadingPositiveOption: GovernancePrimaryTrackOption | null;
      leadingPositiveWeight: GovernanceWeight;
    };
export type GovernanceRecentFinalizedProposal = {
  itemId: GovernanceItemId;
  outcome: GovernanceFinalizedProposalOutcome;
  executionDetail: GovernanceProposalExecutionDetail | null;
};
export type GovernanceGovXpCounters = {
  rollingWinningParticipation: number;
  totalParticipations: bigint;
  totalWinningParticipations: bigint;
  totalAuthoredProposals: bigint;
  totalSuccessfulAuthoredProposals: bigint;
};
export type GovernanceQuerySurfaceAvailability = {
  activeProposalDiscovery: GovernanceQuerySurfaceKind;
  recentFinalizedDiscovery: GovernanceQuerySurfaceKind;
  proposalStatus: GovernanceQuerySurfaceKind;
  proposalMetadata: GovernanceQuerySurfaceKind;
  proposalExecutionAuthority: GovernanceQuerySurfaceKind;
  authorizedRuntimeUpgrade: GovernanceQuerySurfaceKind;
  proposalSubmissionAuthority: GovernanceQuerySurfaceKind;
  proposalOpeningFee: GovernanceQuerySurfaceKind;
  proposalPayloadAvailability: GovernanceQuerySurfaceKind;
  payloadHashPreimageStatus: GovernanceQuerySurfaceKind;
  payloadPreimageNoteCost: GovernanceQuerySurfaceKind;
  proposalPrimaryTrackFamily: GovernanceQuerySurfaceKind;
  proposalPrimaryTrackTally: GovernanceQuerySurfaceKind;
  proposalWinningPrimaryOption: GovernanceQuerySurfaceKind;
  proposalTiming: GovernanceQuerySurfaceKind;
  proposalUrgentEligibility: GovernanceQuerySurfaceKind;
  proposalTally: GovernanceQuerySurfaceKind;
  votePowerProfiles: GovernanceQuerySurfaceKind;
  rewardCoefficient: GovernanceQuerySurfaceKind;
  govxpCounters: GovernanceQuerySurfaceKind;
  proposalExecutionDetail: GovernanceQuerySurfaceKind;
  ballotTimelines: GovernanceQuerySurfaceKind;
  archiveSearch: GovernanceQuerySurfaceKind;
};
export type GovernanceWriteOperation =
  | "castVote"
  | "submitProposal"
  | "noteProposalPreimage"
  | "resolveProposal"
  | "rejectProposal"
  | "resolveProposalFromVotes"
  | "forceResolveProposalFromVotes"
  | "requeueProposalForAutoFinalization";
export type GovernanceWriteAccessKind = "signed" | "admin";
export type GovernanceWriteProviderStatus = "available" | "unavailable";
export type GovernanceProviderStatus = "mock" | TmctolChainConnectionState["status"];
export type GovernanceProviderState = Omit<TmctolChainConnectionState, "status"> & {
  status: GovernanceProviderStatus;
};
export type GovernanceWriteCapability = {
  runtimeAccess: GovernanceWriteAccessKind;
  providerStatus: GovernanceWriteProviderStatus;
  reason: string;
};
export type GovernanceWriteSurfaceAvailability = Record<
  GovernanceWriteOperation,
  GovernanceWriteCapability
>;
export type GovernanceReadAdapter = {
  syncProviderState(): Promise<void>;
  subscribeToUpdates(onUpdate: () => void): () => void;
  getProviderState(): GovernanceProviderState;
  getProviderLabel(): string;
  getQuerySurfaceAvailability(): GovernanceQuerySurfaceAvailability;
  getWriteSurfaceAvailability(): GovernanceWriteSurfaceAvailability;
  getActiveProposalIds(
    domainId: GovernanceDomainId,
  ): Promise<GovernanceItemId[]>;
  getRecentFinalizedProposals(
    domainId: GovernanceDomainId,
  ): Promise<GovernanceRecentFinalizedProposal[]>;
  getProposalStatus(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalStatus | null>;
  getProposalMetadata(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalMetadata | null>;
  getProposalExecutionAuthority(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalExecutionAuthority | null>;
  getAuthorizedRuntimeUpgrade(): Promise<GovernanceAuthorizedRuntimeUpgrade | null>;
  getProposalSubmissionAuthority(
    domainId: GovernanceDomainId,
    payloadKind: GovernanceProposalPayloadKind,
  ): Promise<GovernanceProposalSubmissionAuthority | null>;
  getProposalOpeningFee(
    domainId: GovernanceDomainId,
    payloadKind: GovernanceProposalPayloadKind,
  ): Promise<GovernanceProposalOpeningFee | null>;
  getProposalPayloadAvailability(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalPayloadAvailability | null>;
  getPayloadHashPreimageStatus(
    payloadHash: string,
  ): Promise<GovernancePayloadHashPreimageStatus | null>;
  getPayloadPreimageNoteCost(
    payloadLen: number,
  ): Promise<GovernancePayloadPreimageNoteCost | null>;
  getProposalPrimaryTrackFamily(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalPrimaryTrackFamily | null>;
  getProposalTiming(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalTiming | null>;
  getProposalUrgentEligibility(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<boolean | null>;
  getProposalPrimaryTrackTally(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalPrimaryTrackTally | null>;
  getProposalWinningPrimaryOption(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernancePrimaryTrackOption | null>;
  getProposalTally(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalVoteTally | null>;
  getProposalExecutionDetail(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalExecutionDetail | null>;
  getProposalVotePowerProfile(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
    voteKind: GovernanceVoteKind,
  ): Promise<GovernanceVotePowerProfile | null>;
  getRewardCoefficient(
    domainId: GovernanceDomainId,
    accountId: GovernanceAccountId,
  ): Promise<GovernanceRewardCoefficient | null>;
  getGovXpCounters(
    domainId: GovernanceDomainId,
    accountId: GovernanceAccountId,
  ): Promise<GovernanceGovXpCounters>;
};
export type GovernanceVoteWriteAdapter = {
  castVote(input: {
    accountId: GovernanceAccountId;
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
    voteKind: GovernanceVoteKind;
  }): Promise<void>;
};
export type GovernanceProposalWriteAdapter = {
  submitProposal(input: {
    accountId: GovernanceAccountId;
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
    cadenceMode: GovernanceProposalCadenceMode;
    payloadKind: GovernanceProposalPayloadKind;
    payloadHash: string;
  }): Promise<void>;
  noteProposalPreimage(input: {
    accountId: GovernanceAccountId;
    payloadBytes: Uint8Array;
  }): Promise<void>;
  resolveProposal(input: {
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
    winners: GovernanceAccountId[];
  }): Promise<void>;
  rejectProposal(input: {
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
  }): Promise<void>;
  resolveProposalFromVotes(input: {
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
  }): Promise<void>;
  forceResolveProposalFromVotes(input: {
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
  }): Promise<void>;
  requeueProposalForAutoFinalization(input: {
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
  }): Promise<void>;
};
export type GovernanceAdapter = GovernanceReadAdapter &
  GovernanceVoteWriteAdapter &
  GovernanceProposalWriteAdapter;
export const GOVERNANCE_QUERY_SURFACE_AVAILABILITY: GovernanceQuerySurfaceAvailability = {
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
export const GOVERNANCE_RUNTIME_WRITE_SURFACE: GovernanceWriteSurfaceAvailability = {
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
