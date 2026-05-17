/*
Domain: Web-client public barrel
Owns: Package-level re-export surface for stable cross-domain contracts and selected stores/components.
Excludes: Domain implementation internals, adapter helper internals, and route-local code.
Zone: Root public API; keep minimal and prefer direct domain imports for implementation files.
*/
// Adapters
export {
  BlockchainAdapter,
  connectInjectedSigner,
  DEFAULT_DEOS_DAPP_NAME,
  injectedSignerAvailability,
  injectedSignerExtensionNames,
  type TmctolInjectedSignerAccount,
  type TmctolInjectedSignerAvailability,
  type TmctolInjectedSignerMatch,
} from './adapters/blockchain';
export {
  DEFAULT_DEOS_WS_ENDPOINT,
  DeosPapiConnection,
  normalizeBlockchainEndpoint,
  type DeosChainConnectionState,
  type DeosChainSnapshot,
  type DeosClient,
  type DeosTypedApi,
} from './adapters/blockchain/deos';
export type { Adapter } from './adapters/contract';
export {
  GovernanceMockAdapter,
  GovernancePapiProvider,
  GovernanceRpcProvider,
  GovernanceUnavailableBlockchainProvider,
} from './adapters/governance';
export type { GovernanceBlockchainProvider } from './adapters/governance';
export {
  DEFAULT_GOVERNANCE_RPC_ENDPOINT,
  DEFAULT_GOVERNANCE_WS_ENDPOINT,
  GOVERNANCE_QUERY_SURFACE_AVAILABILITY,
  GOVERNANCE_RUNTIME_WRITE_SURFACE,
  GovernanceMockMaterializedProvider,
  GovernanceUnavailableMaterializedProvider,
} from './governance';
export type {
  GovernanceAccountId,
  GovernanceAdapter,
  GovernanceAuthorizedRuntimeUpgrade,
  GovernanceDomainId,
  GovernanceEpoch,
  GovernanceFinalizedProposalOutcome,
  GovernanceGovXpCounters,
  GovernanceItemId,
  GovernanceMaterializedArchiveEntry,
  GovernanceMaterializedBallotTimelineEntry,
  GovernanceMaterializedProvider,
  GovernancePanelProposal,
  GovernancePayloadHashPreimageStatus,
  GovernancePayloadPreimageNoteCost,
  GovernancePrimaryTrackOption,
  GovernanceProposalCadenceMode,
  GovernanceProposalExecutionAuthority,
  GovernanceProposalExecutionDetail,
  GovernanceProposalExecutionFailureReason,
  GovernanceProposalExecutionSuccessDetail,
  GovernanceProposalMetadata,
  GovernanceProposalOpeningFee,
  GovernanceProposalParameterChangeSurface,
  GovernanceProposalPayloadAvailability,
  GovernanceProposalPayloadKind,
  GovernanceProposalPrimaryTrackFamily,
  GovernanceProposalRejectionReason,
  GovernanceProposalResolutionState,
  GovernanceProposalStatus,
  GovernanceProposalSubmissionAuthority,
  GovernanceProposalTiming,
  GovernanceProposalTreasurySpendScalar,
  GovernanceProposalTreasurySpendSettlementKind,
  GovernanceProposalVoteTally,
  GovernanceProposalWriteAdapter,
  GovernanceProviderState,
  GovernanceProviderStatus,
  GovernancePublicSubmissionOption,
  GovernanceQuerySurfaceAvailability,
  GovernanceQuerySurfaceKind,
  GovernanceReadAdapter,
  GovernanceRecentFinalizedProposal,
  GovernanceRewardCoefficient,
  GovernanceVetoCancellationMode,
  GovernanceViewerState,
  GovernanceVoteKind,
  GovernanceVotePowerProfile,
  GovernanceVoteWriteAdapter,
  GovernanceWeight,
  GovernanceWriteAccessKind,
  GovernanceWriteCapability,
  GovernanceWriteOperation,
  GovernanceWriteProviderStatus,
  GovernanceWriteSurfaceAvailability,
} from './governance';

// Stateful slices and workspace subsystems
export { governanceStore } from './governance/index.svelte';
export { logStore } from './log/index.svelte';
export { marketStore } from './market/index.svelte';
export {
  portfolioStore,
  type KnownClientAsset,
  type TransferAssetKey,
} from './portfolio/index.svelte';

export { layoutStore } from './layout/index.svelte';
export { systemStore } from './system/index.svelte';
export {
  walletStore,
  type WalletAccountOption,
  type WalletAccountSource,
  type WalletSignerStatus,
  type WalletState,
} from './wallet/index.svelte';

// Foundation and domain contracts
export * from './automation/types';
export * from './economics';
export * from './log/types';
export * from './market/types';
export * from './portfolio/types';
export * from './read-model';
export * from './staking/types';
export * from './system/types';
export * from './ui/format';

// Layout subsystem
export { default as AppFooter } from './layout/AppFooter.svelte';
export { default as AppHeader } from './layout/AppHeader.svelte';
export {
  buildLayoutFromSpec,
  CANONICAL_DEFAULT_LAYOUT_SPEC,
  LEGACY_SHIPPED_DEFAULT_LAYOUT_SPEC,
  matchesLayoutSpec,
  type LayoutLeafSpec,
  type LayoutNodeSpec,
  type LayoutSplitSpec,
} from './layout/default-layout';
export { default as PaneHost } from './layout/PaneHost.svelte';
export { default as SidebarPanel } from './layout/SidebarPanel.svelte';
export { default as SplitHandle } from './layout/SplitHandle.svelte';
export { default as TileContainer } from './layout/TileContainer.svelte';
export { default as WorkspaceFrame } from './layout/WorkspaceFrame.svelte';

// Widgets
export * from './widgets/';
