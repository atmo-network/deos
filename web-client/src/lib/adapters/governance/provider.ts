import type {
  GovernanceAccountId,
  GovernanceAuthorizedRuntimeUpgrade,
  GovernanceDomainId,
  GovernanceGovXpCounters,
  GovernanceItemId,
  GovernanceProposalCadenceMode,
  GovernanceProposalExecutionAuthority,
  GovernanceProposalExecutionDetail,
  GovernancePayloadHashPreimageStatus,
  GovernancePayloadPreimageNoteCost,
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
  GovernancePrimaryTrackOption,
  GovernanceProviderState,
  GovernanceQuerySurfaceAvailability,
  GovernanceRecentFinalizedProposal,
  GovernanceRewardCoefficient,
  GovernanceVoteKind,
  GovernanceVotePowerProfile,
  GovernanceWriteSurfaceAvailability,
} from "./types";
import {
  GOVERNANCE_QUERY_SURFACE_AVAILABILITY,
  GOVERNANCE_RUNTIME_WRITE_SURFACE,
} from "./types";

export type GovernanceBlockchainReadProvider = {
  getQuerySurfaceAvailability(): GovernanceQuerySurfaceAvailability;
  getActiveProposalIds(domainId: GovernanceDomainId): Promise<GovernanceItemId[]>;
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

export type GovernanceBlockchainWriteProvider = {
  getWriteSurfaceAvailability(): GovernanceWriteSurfaceAvailability;
  castVote(input: {
    accountId: GovernanceAccountId;
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
    voteKind: GovernanceVoteKind;
  }): Promise<void>;
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

export type GovernanceBlockchainProvider = GovernanceBlockchainReadProvider &
  GovernanceBlockchainWriteProvider & {
    syncProviderState(): Promise<void>;
    subscribeToUpdates(onUpdate: () => void): () => void;
    providerState(): GovernanceProviderState;
    label(): string;
  };

function unavailableWriteSurface(reason: string): GovernanceWriteSurfaceAvailability {
  return {
    castVote: {
      ...GOVERNANCE_RUNTIME_WRITE_SURFACE.castVote,
      reason,
    },
    submitProposal: {
      ...GOVERNANCE_RUNTIME_WRITE_SURFACE.submitProposal,
      reason,
    },
    noteProposalPreimage: {
      ...GOVERNANCE_RUNTIME_WRITE_SURFACE.noteProposalPreimage,
      reason,
    },
    resolveProposal: {
      ...GOVERNANCE_RUNTIME_WRITE_SURFACE.resolveProposal,
      reason,
    },
    rejectProposal: {
      ...GOVERNANCE_RUNTIME_WRITE_SURFACE.rejectProposal,
      reason,
    },
    resolveProposalFromVotes: {
      ...GOVERNANCE_RUNTIME_WRITE_SURFACE.resolveProposalFromVotes,
      reason,
    },
    forceResolveProposalFromVotes: {
      ...GOVERNANCE_RUNTIME_WRITE_SURFACE.forceResolveProposalFromVotes,
      reason,
    },
    requeueProposalForAutoFinalization: {
      ...GOVERNANCE_RUNTIME_WRITE_SURFACE.requeueProposalForAutoFinalization,
      reason,
    },
  };
}

function unconfiguredProviderState(message: string): GovernanceProviderState {
  return {
    status: "unconfigured",
    label: "Blockchain provider (unconfigured)",
    endpoint: null,
    chainName: null,
    nodeName: null,
    nodeVersion: null,
    genesisHash: null,
    finalizedBlockHash: null,
    finalizedBlockNumber: null,
    message,
  };
}

export class GovernanceUnavailableBlockchainProvider
  implements GovernanceBlockchainProvider
{
  private state: GovernanceProviderState;

  constructor(
    private readonly unavailableReason = "No blockchain governance provider configured yet",
  ) {
    this.state = unconfiguredProviderState(unavailableReason);
  }

  async syncProviderState(): Promise<void> {
    this.state = unconfiguredProviderState(this.unavailableReason);
  }

  subscribeToUpdates(): () => void {
    return () => {};
  }

  providerState(): GovernanceProviderState {
    return this.state;
  }

  label(): string {
    return this.state.label;
  }

  getQuerySurfaceAvailability(): GovernanceQuerySurfaceAvailability {
    return GOVERNANCE_QUERY_SURFACE_AVAILABILITY;
  }

  getWriteSurfaceAvailability(): GovernanceWriteSurfaceAvailability {
    return unavailableWriteSurface(this.unavailableReason);
  }

  async getActiveProposalIds(_domainId: GovernanceDomainId): Promise<GovernanceItemId[]> {
    throw new Error(this.unavailableReason);
  }

  async getRecentFinalizedProposals(
    _domainId: GovernanceDomainId,
  ): Promise<GovernanceRecentFinalizedProposal[]> {
    throw new Error(this.unavailableReason);
  }

  async getProposalStatus(
    _domainId: GovernanceDomainId,
    _itemId: GovernanceItemId,
  ): Promise<GovernanceProposalStatus | null> {
    throw new Error(this.unavailableReason);
  }

  async getProposalMetadata(
    _domainId: GovernanceDomainId,
    _itemId: GovernanceItemId,
  ): Promise<GovernanceProposalMetadata | null> {
    throw new Error(this.unavailableReason);
  }

  async getProposalExecutionAuthority(
    _domainId: GovernanceDomainId,
    _itemId: GovernanceItemId,
  ): Promise<GovernanceProposalExecutionAuthority | null> {
    throw new Error(this.unavailableReason);
  }

  async getAuthorizedRuntimeUpgrade(): Promise<GovernanceAuthorizedRuntimeUpgrade | null> {
    throw new Error(this.unavailableReason);
  }

  async getProposalSubmissionAuthority(
    _domainId: GovernanceDomainId,
    _payloadKind: GovernanceProposalPayloadKind,
  ): Promise<GovernanceProposalSubmissionAuthority | null> {
    throw new Error(this.unavailableReason);
  }

  async getProposalOpeningFee(
    _domainId: GovernanceDomainId,
    _payloadKind: GovernanceProposalPayloadKind,
  ): Promise<GovernanceProposalOpeningFee | null> {
    throw new Error(this.unavailableReason);
  }

  async getProposalPayloadAvailability(
    _domainId: GovernanceDomainId,
    _itemId: GovernanceItemId,
  ): Promise<GovernanceProposalPayloadAvailability | null> {
    throw new Error(this.unavailableReason);
  }

  async getPayloadHashPreimageStatus(): Promise<GovernancePayloadHashPreimageStatus | null> {
    throw new Error(this.unavailableReason);
  }

  async getPayloadPreimageNoteCost(): Promise<GovernancePayloadPreimageNoteCost | null> {
    throw new Error(this.unavailableReason);
  }

  async getProposalPrimaryTrackFamily(
    _domainId: GovernanceDomainId,
    _itemId: GovernanceItemId,
  ): Promise<GovernanceProposalPrimaryTrackFamily | null> {
    throw new Error(this.unavailableReason);
  }

  async getProposalTiming(
    _domainId: GovernanceDomainId,
    _itemId: GovernanceItemId,
  ): Promise<GovernanceProposalTiming | null> {
    throw new Error(this.unavailableReason);
  }

  async getProposalUrgentEligibility(
    _domainId: GovernanceDomainId,
    _itemId: GovernanceItemId,
  ): Promise<boolean | null> {
    throw new Error(this.unavailableReason);
  }

  async getProposalPrimaryTrackTally(
    _domainId: GovernanceDomainId,
    _itemId: GovernanceItemId,
  ): Promise<GovernanceProposalPrimaryTrackTally | null> {
    throw new Error(this.unavailableReason);
  }

  async getProposalWinningPrimaryOption(
    _domainId: GovernanceDomainId,
    _itemId: GovernanceItemId,
  ): Promise<GovernancePrimaryTrackOption | null> {
    throw new Error(this.unavailableReason);
  }

  async getProposalTally(
    _domainId: GovernanceDomainId,
    _itemId: GovernanceItemId,
  ): Promise<GovernanceProposalVoteTally | null> {
    throw new Error(this.unavailableReason);
  }

  async getProposalExecutionDetail(
    _domainId: GovernanceDomainId,
    _itemId: GovernanceItemId,
  ): Promise<GovernanceProposalExecutionDetail | null> {
    throw new Error(this.unavailableReason);
  }

  async getProposalVotePowerProfile(
    _domainId: GovernanceDomainId,
    _itemId: GovernanceItemId,
    _voteKind: GovernanceVoteKind,
  ): Promise<GovernanceVotePowerProfile | null> {
    throw new Error(this.unavailableReason);
  }

  async getRewardCoefficient(
    _domainId: GovernanceDomainId,
    _accountId: GovernanceAccountId,
  ): Promise<GovernanceRewardCoefficient | null> {
    throw new Error(this.unavailableReason);
  }

  async getGovXpCounters(
    _domainId: GovernanceDomainId,
    _accountId: GovernanceAccountId,
  ): Promise<GovernanceGovXpCounters> {
    throw new Error(this.unavailableReason);
  }

  async castVote(): Promise<void> {
    throw new Error(this.unavailableReason);
  }

  async submitProposal(_input: {
    accountId: GovernanceAccountId;
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
    cadenceMode: GovernanceProposalCadenceMode;
    payloadKind: GovernanceProposalPayloadKind;
    payloadHash: string;
  }): Promise<void> {
    throw new Error(this.unavailableReason);
  }

  async noteProposalPreimage(): Promise<void> {
    throw new Error(this.unavailableReason);
  }

  async resolveProposal(): Promise<void> {
    throw new Error(this.unavailableReason);
  }

  async rejectProposal(): Promise<void> {
    throw new Error(this.unavailableReason);
  }

  async resolveProposalFromVotes(): Promise<void> {
    throw new Error(this.unavailableReason);
  }

  async forceResolveProposalFromVotes(): Promise<void> {
    throw new Error(this.unavailableReason);
  }

  async requeueProposalForAutoFinalization(): Promise<void> {
    throw new Error(this.unavailableReason);
  }
}
