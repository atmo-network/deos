/*
Domain: Governance provider contract
Owns: Governance blockchain-provider interface and unavailable-provider fallback behavior.
Excludes: Concrete PAPI/RPC implementation, mock fixture data, store lifecycle, and UI rendering.
Zone: Adapter contract boundary for governance; imported by governance store and provider implementations.
*/
import type {
  GovernanceAccountId,
  GovernanceAccountPowerView,
  GovernanceAdapter,
  GovernanceAuthorizedRuntimeUpgrade,
  GovernanceDomainId,
  GovernanceGovXpCounters,
  GovernanceItemId,
  GovernancePayloadHashPreimageStatus,
  GovernancePayloadPreimageNoteCost,
  GovernancePrimaryTrackOption,
  GovernanceProposalCadenceMode,
  GovernanceProposalExecutionAuthority,
  GovernanceProposalExecutionDetail,
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
  GovernanceQuerySurfaceAvailability,
  GovernanceRecentFinalizedProposal,
  GovernanceRewardCoefficient,
  GovernanceVoteKind,
  GovernanceVotePowerProfile,
  GovernanceWriteSurfaceAvailability,
} from '$lib/governance';
import {
  GOVERNANCE_QUERY_SURFACE_AVAILABILITY,
  buildWriteSurfaceAvailability,
} from '$lib/governance';

export type GovernanceBlockchainProvider = GovernanceAdapter;

function unavailableWriteSurface(
  reason: string,
): GovernanceWriteSurfaceAvailability {
  return buildWriteSurfaceAvailability({
    castVote: { reason },
    submitProposal: { reason },
    noteProposalPreimage: { reason },
    resolveProposal: { reason },
    rejectProposal: { reason },
    resolveProposalFromVotes: { reason },
    forceResolveProposalFromVotes: { reason },
    requeueProposalForAutoFinalization: { reason },
  });
}

function unconfiguredProviderState(message: string): GovernanceProviderState {
  return {
    status: 'unconfigured',
    label: 'Blockchain provider (unconfigured)',
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

export class GovernanceUnavailableBlockchainProvider implements GovernanceBlockchainProvider {
  private state: GovernanceProviderState;

  constructor(
    private readonly unavailableReason = 'No blockchain governance provider configured yet',
  ) {
    this.state = unconfiguredProviderState(unavailableReason);
  }

  async syncProviderState(): Promise<void> {
    this.state = unconfiguredProviderState(this.unavailableReason);
  }

  subscribeToUpdates(): () => void {
    return () => {};
  }

  getProviderState(): GovernanceProviderState {
    return this.state;
  }

  getProviderLabel(): string {
    return this.state.label;
  }

  getQuerySurfaceAvailability(): GovernanceQuerySurfaceAvailability {
    return GOVERNANCE_QUERY_SURFACE_AVAILABILITY;
  }

  getWriteSurfaceAvailability(
    _accountId?: GovernanceAccountId | null,
  ): GovernanceWriteSurfaceAvailability {
    return unavailableWriteSurface(this.unavailableReason);
  }

  async getActiveProposalIds(
    _domainId: GovernanceDomainId,
  ): Promise<GovernanceItemId[]> {
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

  async getAccountGovernancePowerView(
    _domainId: GovernanceDomainId,
    _itemId: GovernanceItemId,
    _accountId: GovernanceAccountId,
  ): Promise<GovernanceAccountPowerView | null> {
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
