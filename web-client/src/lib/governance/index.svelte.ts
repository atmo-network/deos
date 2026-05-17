/*
Domain: Governance store
Owns: Governance query/write state, proposal drafts, vote/preimage flows, and provider-backed refresh lifecycle.
Excludes: Adapter RPC internals, wallet signer custody, layout state, and reusable UI primitives.
Zone: Governance state slice; composes governance provider contracts with local session/draft state.
*/
import {
  type GovernanceBlockchainProvider,
  GovernanceUnavailableBlockchainProvider,
} from '$lib/adapters/governance/provider';
import type {
  GovernanceAdapter,
  GovernancePrimaryTrackOption,
  GovernanceProposalDescriptor,
  GovernanceProposalExecutionDetail,
  GovernanceProposalPayloadKind,
  GovernanceVoteKind,
} from '$lib/governance';
import {
  getGovernanceDomainId,
  setGovernanceDomainId,
} from '$lib/governance/session';
import { logStore } from '$lib/log/index.svelte';
import { fromClientBoundedProjection } from '$lib/read-model';
import {
  getBlockchainEndpoint,
  setBlockchainEndpoint,
} from '$lib/system/endpoint';
import { walletStore } from '$lib/wallet/index.svelte';

import type {
  GovernancePanelProposal,
  GovernancePublicSubmissionOption,
  GovernanceRetainedFinalizedProposal,
  GovernanceViewerState,
} from './types';

const DEFAULT_ACCOUNT_ID = walletStore.selectedAddress;
const DEFAULT_BLOCKCHAIN_PROVIDER = new GovernanceUnavailableBlockchainProvider(
  'Governance provider not initialized yet',
);

function buildGovernanceAdapter(
  blockchainProvider: GovernanceBlockchainProvider,
): GovernanceAdapter {
  return blockchainProvider;
}

const DEFAULT_ADAPTER = buildGovernanceAdapter(DEFAULT_BLOCKCHAIN_PROVIDER);
const PUBLIC_SUBMISSION_PAYLOAD_KINDS: GovernanceProposalPayloadKind[] = [
  'Intent',
  'L2SignalToL1',
  'L1RootAction',
  'L2TreasurySpend',
  'L2ParameterChange',
];

async function loadPublicSubmissionOptions(
  adapter: GovernanceAdapter,
  domainId: number,
): Promise<GovernancePublicSubmissionOption[]> {
  const entries = await Promise.all(
    PUBLIC_SUBMISSION_PAYLOAD_KINDS.map(async (payloadKind) => ({
      payloadKind,
      authority: await adapter.getProposalSubmissionAuthority(
        domainId,
        payloadKind,
      ),
      openingFee: await adapter.getProposalOpeningFee(domainId, payloadKind),
    })),
  );
  return entries
    .filter((entry) => entry.authority === 'Signed')
    .map((entry) => ({
      payloadKind: entry.payloadKind,
      openingFee: entry.openingFee,
    }));
}

async function loadProposalCommonFields(
  adapter: GovernanceAdapter,
  domainId: number,
  itemId: number,
): Promise<GovernanceProposalDescriptor> {
  const [
    metadata,
    executionAuthority,
    payloadAvailability,
    primaryTrackFamily,
    urgentEligibility,
  ] = await Promise.all([
    adapter.getProposalMetadata(domainId, itemId),
    adapter.getProposalExecutionAuthority(domainId, itemId),
    adapter.getProposalPayloadAvailability(domainId, itemId),
    adapter.getProposalPrimaryTrackFamily(domainId, itemId),
    adapter.getProposalUrgentEligibility(domainId, itemId),
  ]);
  const submissionAuthority = metadata
    ? await adapter.getProposalSubmissionAuthority(
        domainId,
        metadata.payloadKind,
      )
    : null;
  const openingFee = metadata
    ? await adapter.getProposalOpeningFee(domainId, metadata.payloadKind)
    : null;
  return {
    metadata,
    executionAuthority,
    payloadAvailability,
    primaryTrackFamily,
    urgentEligibility,
    submissionAuthority,
    openingFee,
  };
}

async function loadProposal(
  adapter: GovernanceAdapter,
  domainId: number,
  itemId: number,
  accountId: string,
): Promise<GovernancePanelProposal> {
  const common = await loadProposalCommonFields(adapter, domainId, itemId);
  const [
    status,
    timing,
    primaryTrackTally,
    tally,
    accountPowerView,
    aye,
    nay,
    amplify,
    approve,
    reduce,
    veto,
    pass,
  ] = await Promise.all([
    adapter.getProposalStatus(domainId, itemId),
    adapter.getProposalTiming(domainId, itemId),
    adapter.getProposalPrimaryTrackTally(domainId, itemId),
    adapter.getProposalTally(domainId, itemId),
    adapter.getAccountGovernancePowerView(domainId, itemId, accountId),
    adapter.getProposalVotePowerProfile(domainId, itemId, 'aye'),
    adapter.getProposalVotePowerProfile(domainId, itemId, 'nay'),
    adapter.getProposalVotePowerProfile(domainId, itemId, 'amplify'),
    adapter.getProposalVotePowerProfile(domainId, itemId, 'approve'),
    adapter.getProposalVotePowerProfile(domainId, itemId, 'reduce'),
    adapter.getProposalVotePowerProfile(domainId, itemId, 'veto'),
    adapter.getProposalVotePowerProfile(domainId, itemId, 'pass'),
  ]);
  return {
    itemId,
    status,
    ...common,
    timing,
    primaryTrackTally,
    tally,
    accountPowerView,
    votePowerProfiles: {
      aye: aye ?? undefined,
      nay: nay ?? undefined,
      amplify: amplify ?? undefined,
      approve: approve ?? undefined,
      reduce: reduce ?? undefined,
      veto: veto ?? undefined,
      pass: pass ?? undefined,
    },
  };
}

function emptyGovXpCounters() {
  return {
    rollingWinningParticipation: 0,
    totalParticipations: 0n,
    totalWinningParticipations: 0n,
    totalAuthoredProposals: 0n,
    totalSuccessfulAuthoredProposals: 0n,
  };
}

class GovernanceStore {
  private initialized = false;
  private refreshInFlight: Promise<void> | null = null;
  private refreshQueued = false;
  private unsubscribeUpdates: () => void = () => {};
  adapter: GovernanceAdapter = $state(DEFAULT_ADAPTER);
  state: GovernanceViewerState = $state({
    providerState: DEFAULT_ADAPTER.getProviderState(),
    endpoint: getBlockchainEndpoint(),
    domainId: getGovernanceDomainId(),
    accountId: DEFAULT_ACCOUNT_ID,
    activeProposalIds: [],
    activeProposals: [],
    submissionOptions: [],
    authorizedRuntimeUpgrade: null,
    recentFinalizedProposals: [],
    recentFinalizedProposalsView: null,
    rewardCoefficient: null,
    govxpCounters: emptyGovXpCounters(),
    loading: false,
    error: null,
    writeError: null,
    writeSurfaceAvailability: DEFAULT_ADAPTER.getWriteSurfaceAvailability(
      walletStore.selectedAddress,
    ),
  });

  private bindAdapterUpdates() {
    this.unsubscribeUpdates();
    this.unsubscribeUpdates = this.adapter.subscribeToUpdates(() => {
      void this.refresh();
    });
  }

  private clearReadState() {
    this.state.activeProposalIds = [];
    this.state.activeProposals = [];
    this.state.submissionOptions = [];
    this.state.authorizedRuntimeUpgrade = null;
    this.state.recentFinalizedProposals = [];
    this.state.recentFinalizedProposalsView = null;
    this.state.rewardCoefficient = null;
    this.state.govxpCounters = emptyGovXpCounters();
  }

  private async loadBlockchainProvider(endpoint: string) {
    const normalizedEndpoint = endpoint.trim();
    if (normalizedEndpoint.length === 0) {
      this.setBlockchainProvider(
        new GovernanceUnavailableBlockchainProvider(
          'No blockchain PAPI endpoint configured',
        ),
      );
      return;
    }
    const { GovernancePapiProvider } =
      await import('$lib/adapters/governance/papi');
    if (this.state.endpoint.trim() !== normalizedEndpoint) {
      return;
    }
    this.setBlockchainProvider(new GovernancePapiProvider(normalizedEndpoint));
  }

  async init() {
    if (this.initialized) {
      return;
    }
    this.initialized = true;
    await this.loadBlockchainProvider(this.state.endpoint);
    await this.refresh();
  }

  async refresh() {
    if (this.refreshInFlight) {
      this.refreshQueued = true;
      await this.refreshInFlight;
      return;
    }
    this.refreshInFlight = this.performRefresh();
    await this.refreshInFlight;
    this.refreshInFlight = null;
    if (this.refreshQueued) {
      this.refreshQueued = false;
      await this.refresh();
    }
  }

  private clearWriteError() {
    this.state.writeError = null;
  }

  private recordWriteError(message: string) {
    this.state.writeError = message;
    logStore.add(message, 'error', {
      blockNumber: null,
    });
  }

  private async runWrite(action: () => Promise<void>, fallbackMessage: string) {
    try {
      await action();
      this.clearWriteError();
    } catch (error) {
      this.recordWriteError(
        error instanceof Error ? error.message : fallbackMessage,
      );
    }
    await this.refresh();
  }

  private async performRefresh() {
    this.state.loading = true;
    this.state.error = null;
    try {
      this.state.accountId = walletStore.selectedAddress;
      await this.adapter.syncProviderState();
      this.state.providerState = this.adapter.getProviderState();
      this.state.writeSurfaceAvailability =
        this.adapter.getWriteSurfaceAvailability(this.state.accountId);
      if (this.state.providerState.status !== 'connected') {
        this.clearReadState();
        return;
      }
      const [
        activeProposalIds,
        submissionOptions,
        authorizedRuntimeUpgrade,
        recentFinalizedProposals,
        rewardCoefficient,
        govxpCounters,
      ] = await Promise.all([
        this.adapter.getActiveProposalIds(this.state.domainId),
        loadPublicSubmissionOptions(this.adapter, this.state.domainId),
        this.adapter.getAuthorizedRuntimeUpgrade(),
        this.adapter.getRecentFinalizedProposals(this.state.domainId),
        this.adapter.getRewardCoefficient(
          this.state.domainId,
          this.state.accountId,
        ),
        this.adapter.getGovXpCounters(
          this.state.domainId,
          this.state.accountId,
        ),
      ]);
      const activeProposals = await Promise.all(
        activeProposalIds.map((itemId) =>
          loadProposal(
            this.adapter,
            this.state.domainId,
            itemId,
            this.state.accountId,
          ),
        ),
      );
      const recentRetainedDetails = await Promise.all(
        recentFinalizedProposals.map(async (proposal) => {
          const [executionDetail, winningPrimaryOption, common] =
            await Promise.all([
              this.adapter.getProposalExecutionDetail(
                this.state.domainId,
                proposal.itemId,
              ),
              this.adapter.getProposalWinningPrimaryOption(
                this.state.domainId,
                proposal.itemId,
              ),
              loadProposalCommonFields(
                this.adapter,
                this.state.domainId,
                proposal.itemId,
              ),
            ]);
          return {
            itemId: proposal.itemId,
            executionDetail,
            winningPrimaryOption,
            ...common,
          };
        }),
      );
      const retainedDetailByItem = new Map<
        number,
        GovernanceProposalDescriptor & {
          executionDetail: GovernanceProposalExecutionDetail | null;
          winningPrimaryOption: GovernancePrimaryTrackOption | null;
        }
      >(
        recentRetainedDetails.map((entry) => [
          entry.itemId,
          {
            executionDetail: entry.executionDetail,
            winningPrimaryOption: entry.winningPrimaryOption,
            metadata: entry.metadata,
            executionAuthority: entry.executionAuthority,
            payloadAvailability: entry.payloadAvailability,
            primaryTrackFamily: entry.primaryTrackFamily,
            urgentEligibility: entry.urgentEligibility,
            submissionAuthority: entry.submissionAuthority,
            openingFee: entry.openingFee,
          },
        ]),
      );
      const recentFinalizedWithDetail: GovernanceRetainedFinalizedProposal[] =
        recentFinalizedProposals.map((proposal) => {
          const retainedDetail = retainedDetailByItem.get(proposal.itemId);
          return {
            ...proposal,
            executionDetail:
              retainedDetail?.executionDetail ??
              proposal.executionDetail ??
              null,
            metadata: retainedDetail?.metadata ?? null,
            executionAuthority: retainedDetail?.executionAuthority ?? null,
            payloadAvailability: retainedDetail?.payloadAvailability ?? null,
            primaryTrackFamily: retainedDetail?.primaryTrackFamily ?? null,
            winningPrimaryOption: retainedDetail?.winningPrimaryOption ?? null,
            urgentEligibility: retainedDetail?.urgentEligibility ?? null,
            submissionAuthority: retainedDetail?.submissionAuthority ?? null,
            openingFee: retainedDetail?.openingFee ?? null,
          };
        });
      this.state.activeProposalIds = activeProposalIds;
      this.state.activeProposals = activeProposals;
      this.state.submissionOptions = submissionOptions;
      this.state.authorizedRuntimeUpgrade = authorizedRuntimeUpgrade;
      this.state.recentFinalizedProposals = recentFinalizedWithDetail;
      this.state.recentFinalizedProposalsView = fromClientBoundedProjection(
        recentFinalizedWithDetail,
        'governanceStore.recentFinalizedProposals <- Governance.recent_finalized_proposals + Governance.proposal_metadata + Governance.proposal_execution_authority + Governance.proposal_payload_availability + Governance.proposal_execution_detail + Governance.retained_proposal_winning_primary_option',
        'bounded-recent',
      );
      this.state.rewardCoefficient = rewardCoefficient;
      this.state.govxpCounters = govxpCounters;
    } catch (error) {
      this.clearReadState();
      this.state.error =
        error instanceof Error
          ? error.message
          : 'Unknown governance adapter error';
    } finally {
      this.state.loading = false;
    }
  }

  setBlockchainProvider(provider: GovernanceBlockchainProvider) {
    this.adapter = buildGovernanceAdapter(provider);
    this.bindAdapterUpdates();
  }

  async setEndpoint(endpoint: string) {
    this.state.endpoint = endpoint.trim();
    setBlockchainEndpoint(this.state.endpoint);
    this.clearWriteError();
    await this.loadBlockchainProvider(this.state.endpoint);
  }

  setDomainId(domainId: number) {
    this.state.domainId = domainId;
    setGovernanceDomainId(domainId);
    this.clearWriteError();
  }

  async castVote(itemId: number, voteKind: GovernanceVoteKind) {
    await this.runWrite(
      () =>
        this.adapter.castVote({
          accountId: walletStore.selectedAddress,
          domainId: this.state.domainId,
          itemId,
          voteKind,
        }),
      'Unknown governance vote error',
    );
  }

  async lookupPayloadHashPreimageStatus(payloadHash: string) {
    return this.adapter.getPayloadHashPreimageStatus(payloadHash);
  }

  async lookupPayloadPreimageNoteCost(payloadLen: number) {
    return this.adapter.getPayloadPreimageNoteCost(payloadLen);
  }

  async submitProposal(input: {
    itemId: number;
    cadenceMode: 'Ordinary' | 'Fast';
    payloadKind:
      | 'L1RootAction'
      | 'L2TreasurySpend'
      | 'L2ParameterChange'
      | 'Intent'
      | 'L2SignalToL1';
    payloadHash: string;
  }) {
    await this.runWrite(
      () =>
        this.adapter.submitProposal({
          accountId: walletStore.selectedAddress,
          domainId: this.state.domainId,
          itemId: input.itemId,
          cadenceMode: input.cadenceMode,
          payloadKind: input.payloadKind,
          payloadHash: input.payloadHash,
        }),
      'Unknown governance proposal submission error',
    );
  }

  async noteProposalPreimage(payloadBytes: Uint8Array) {
    await this.runWrite(
      () =>
        this.adapter.noteProposalPreimage({
          accountId: walletStore.selectedAddress,
          payloadBytes,
        }),
      'Unknown governance preimage note error',
    );
  }

  async rejectProposal(itemId: number) {
    await this.runWrite(
      () =>
        this.adapter.rejectProposal({
          domainId: this.state.domainId,
          itemId,
        }),
      'Unknown governance proposal rejection error',
    );
  }

  async resolveProposal(itemId: number, winners: string[]) {
    await this.runWrite(
      () =>
        this.adapter.resolveProposal({
          domainId: this.state.domainId,
          itemId,
          winners,
        }),
      'Unknown governance proposal resolution error',
    );
  }

  async resolveProposalFromVotes(itemId: number) {
    await this.runWrite(
      () =>
        this.adapter.resolveProposalFromVotes({
          domainId: this.state.domainId,
          itemId,
        }),
      'Unknown governance vote resolution error',
    );
  }

  async forceResolveProposalFromVotes(itemId: number) {
    await this.runWrite(
      () =>
        this.adapter.forceResolveProposalFromVotes({
          domainId: this.state.domainId,
          itemId,
        }),
      'Unknown forced governance vote resolution error',
    );
  }

  async requeueProposalForAutoFinalization(itemId: number) {
    await this.runWrite(
      () =>
        this.adapter.requeueProposalForAutoFinalization({
          domainId: this.state.domainId,
          itemId,
        }),
      'Unknown governance requeue error',
    );
  }
}

export const governanceStore = new GovernanceStore();
