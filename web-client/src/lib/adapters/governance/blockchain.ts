import type {
  GovernanceAdapter,
  GovernanceAuthorizedRuntimeUpgrade,
  GovernanceDomainId,
  GovernanceItemId,
  GovernanceProposalCadenceMode,
  GovernanceProposalExecutionAuthority,
  GovernanceProposalExecutionDetail,
  GovernancePayloadHashPreimageStatus,
  GovernancePayloadPreimageNoteCost,
  GovernanceProposalOpeningFee,
  GovernanceProposalPayloadKind,
  GovernanceProposalSubmissionAuthority,
  GovernanceProposalMetadata,
  GovernanceProposalPrimaryTrackFamily,
  GovernanceProposalPrimaryTrackTally,
  GovernancePrimaryTrackOption,
  GovernanceProposalPayloadAvailability,
  GovernanceProposalStatus,
  GovernanceProposalTiming,
  GovernanceProposalVoteTally,
  GovernanceProviderState,
  GovernanceQuerySurfaceAvailability,
  GovernanceRecentFinalizedProposal,
  GovernanceGovXpCounters,
  GovernanceVoteKind,
  GovernanceVotePowerProfile,
  GovernanceRewardCoefficient,
  GovernanceAccountId,
  GovernanceWriteSurfaceAvailability,
} from "./types";
import {
  GovernanceUnavailableBlockchainProvider,
  type GovernanceBlockchainProvider,
} from "./provider";

export class GovernanceBlockchainAdapter implements GovernanceAdapter {
  constructor(
    private readonly provider: GovernanceBlockchainProvider = new GovernanceUnavailableBlockchainProvider(),
  ) {}

  async syncProviderState(): Promise<void> {
    await this.provider.syncProviderState();
  }

  subscribeToUpdates(onUpdate: () => void): () => void {
    return this.provider.subscribeToUpdates(onUpdate);
  }

  getProviderState(): GovernanceProviderState {
    return this.provider.providerState();
  }

  getProviderLabel(): string {
    return this.provider.label();
  }

  getQuerySurfaceAvailability(): GovernanceQuerySurfaceAvailability {
    return this.provider.getQuerySurfaceAvailability();
  }

  getWriteSurfaceAvailability(): GovernanceWriteSurfaceAvailability {
    return this.provider.getWriteSurfaceAvailability();
  }

  async getActiveProposalIds(domainId: GovernanceDomainId): Promise<GovernanceItemId[]> {
    return this.provider.getActiveProposalIds(domainId);
  }

  async getRecentFinalizedProposals(
    domainId: GovernanceDomainId,
  ): Promise<GovernanceRecentFinalizedProposal[]> {
    return this.provider.getRecentFinalizedProposals(domainId);
  }

  async getProposalStatus(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalStatus | null> {
    return this.provider.getProposalStatus(domainId, itemId);
  }

  async getProposalMetadata(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalMetadata | null> {
    return this.provider.getProposalMetadata(domainId, itemId);
  }

  async getProposalExecutionAuthority(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalExecutionAuthority | null> {
    return this.provider.getProposalExecutionAuthority(domainId, itemId);
  }

  async getAuthorizedRuntimeUpgrade(): Promise<GovernanceAuthorizedRuntimeUpgrade | null> {
    return this.provider.getAuthorizedRuntimeUpgrade();
  }

  async getProposalSubmissionAuthority(
    domainId: GovernanceDomainId,
    payloadKind: GovernanceProposalPayloadKind,
  ): Promise<GovernanceProposalSubmissionAuthority | null> {
    return this.provider.getProposalSubmissionAuthority(domainId, payloadKind);
  }

  async getProposalOpeningFee(
    domainId: GovernanceDomainId,
    payloadKind: GovernanceProposalPayloadKind,
  ): Promise<GovernanceProposalOpeningFee | null> {
    return this.provider.getProposalOpeningFee(domainId, payloadKind);
  }

  async getProposalPayloadAvailability(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalPayloadAvailability | null> {
    return this.provider.getProposalPayloadAvailability(domainId, itemId);
  }

  async getPayloadHashPreimageStatus(
    payloadHash: string,
  ): Promise<GovernancePayloadHashPreimageStatus | null> {
    return this.provider.getPayloadHashPreimageStatus(payloadHash);
  }

  async getPayloadPreimageNoteCost(
    payloadLen: number,
  ): Promise<GovernancePayloadPreimageNoteCost | null> {
    return this.provider.getPayloadPreimageNoteCost(payloadLen);
  }

  async getProposalPrimaryTrackFamily(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalPrimaryTrackFamily | null> {
    return this.provider.getProposalPrimaryTrackFamily(domainId, itemId);
  }

  async getProposalTiming(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalTiming | null> {
    return this.provider.getProposalTiming(domainId, itemId);
  }

  async getProposalUrgentEligibility(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<boolean | null> {
    return this.provider.getProposalUrgentEligibility(domainId, itemId);
  }

  async getProposalPrimaryTrackTally(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalPrimaryTrackTally | null> {
    return this.provider.getProposalPrimaryTrackTally(domainId, itemId);
  }

  async getProposalWinningPrimaryOption(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernancePrimaryTrackOption | null> {
    return this.provider.getProposalWinningPrimaryOption(domainId, itemId);
  }

  async getProposalTally(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalVoteTally | null> {
    return this.provider.getProposalTally(domainId, itemId);
  }

  async getProposalExecutionDetail(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalExecutionDetail | null> {
    return this.provider.getProposalExecutionDetail(domainId, itemId);
  }

  async getProposalVotePowerProfile(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
    voteKind: GovernanceVoteKind,
  ): Promise<GovernanceVotePowerProfile | null> {
    return this.provider.getProposalVotePowerProfile(domainId, itemId, voteKind);
  }

  async getRewardCoefficient(
    domainId: GovernanceDomainId,
    accountId: GovernanceAccountId,
  ): Promise<GovernanceRewardCoefficient | null> {
    return this.provider.getRewardCoefficient(domainId, accountId);
  }

  async getGovXpCounters(
    domainId: GovernanceDomainId,
    accountId: GovernanceAccountId,
  ): Promise<GovernanceGovXpCounters> {
    return this.provider.getGovXpCounters(domainId, accountId);
  }

  async castVote(input: {
    accountId: GovernanceAccountId;
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
    voteKind: GovernanceVoteKind;
  }): Promise<void> {
    return this.provider.castVote(input);
  }

  async submitProposal(input: {
    accountId: GovernanceAccountId;
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
    cadenceMode: GovernanceProposalCadenceMode;
    payloadKind: GovernanceProposalPayloadKind;
    payloadHash: string;
  }): Promise<void> {
    return this.provider.submitProposal(input);
  }

  async noteProposalPreimage(input: {
    accountId: GovernanceAccountId;
    payloadBytes: Uint8Array;
  }): Promise<void> {
    return this.provider.noteProposalPreimage(input);
  }

  async resolveProposal(input: {
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
    winners: GovernanceAccountId[];
  }): Promise<void> {
    return this.provider.resolveProposal(input);
  }

  async rejectProposal(input: {
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
  }): Promise<void> {
    return this.provider.rejectProposal(input);
  }

  async resolveProposalFromVotes(input: {
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
  }): Promise<void> {
    return this.provider.resolveProposalFromVotes(input);
  }

  async forceResolveProposalFromVotes(input: {
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
  }): Promise<void> {
    return this.provider.forceResolveProposalFromVotes(input);
  }

  async requeueProposalForAutoFinalization(input: {
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
  }): Promise<void> {
    return this.provider.requeueProposalForAutoFinalization(input);
  }
}
