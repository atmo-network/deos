// Adapters
export type { Adapter } from "./adapters/types";
export type {
  GovernanceAccountId,
  GovernanceAdapter,
  GovernanceAuthorizedRuntimeUpgrade,
  GovernanceProposalWriteAdapter,
  GovernanceDomainId,
  GovernanceEpoch,
  GovernanceFinalizedProposalOutcome,
  GovernanceGovXpCounters,
  GovernanceItemId,
  GovernanceMaterializedArchiveEntry,
  GovernanceMaterializedBallotTimelineEntry,
  GovernanceMaterializedProvider,
  GovernanceProposalCadenceMode,
  GovernanceProposalExecutionAuthority,
  GovernanceProposalOpeningFee,
  GovernanceProposalSubmissionAuthority,
  GovernanceProposalExecutionDetail,
  GovernanceProposalExecutionFailureReason,
  GovernanceProposalMetadata,
  GovernancePayloadHashPreimageStatus,
  GovernancePayloadPreimageNoteCost,
  GovernanceProposalPayloadAvailability,
  GovernanceProposalPrimaryTrackFamily,
  GovernancePrimaryTrackOption,
  GovernanceProposalTiming,
  GovernanceProposalExecutionSuccessDetail,
  GovernanceProposalParameterChangeSurface,
  GovernanceProposalTreasurySpendScalar,
  GovernanceProposalPayloadKind,
  GovernanceProposalRejectionReason,
  GovernanceProposalResolutionState,
  GovernanceProposalStatus,
  GovernanceProposalTreasurySpendSettlementKind,
  GovernanceProposalVoteTally,
  GovernanceProviderState,
  GovernanceProviderStatus,
  GovernanceQuerySurfaceAvailability,
  GovernanceQuerySurfaceKind,
  GovernanceReadAdapter,
  GovernanceRecentFinalizedProposal,
  GovernanceRewardCoefficient,
  GovernanceVetoCancellationMode,
  GovernanceVoteKind,
  GovernanceVotePowerProfile,
  GovernanceVoteWriteAdapter,
  GovernanceWeight,
  GovernanceWriteAccessKind,
  GovernanceWriteCapability,
  GovernanceWriteOperation,
  GovernanceWriteProviderStatus,
  GovernanceWriteSurfaceAvailability,
  GovernanceBlockchainProvider,
  GovernanceBlockchainReadProvider,
  GovernanceBlockchainWriteProvider,
} from "./adapters/governance";
export {
  BlockchainAdapter,
  connectInjectedSigner,
  DEFAULT_TMCTOL_DAPP_NAME,
  injectedSignerAvailability,
  injectedSignerExtensionNames,
  type TmctolInjectedSignerAccount,
  type TmctolInjectedSignerAvailability,
  type TmctolInjectedSignerMatch,
} from "./adapters/blockchain";
export {
  DEFAULT_TMCTOL_WS_ENDPOINT,
  TmctolPapiConnection,
  normalizeBlockchainEndpoint,
  type TmctolChainConnectionState,
  type TmctolChainSnapshot,
  type TmctolClient,
  type TmctolTypedApi,
} from "./adapters/blockchain/deos";
export {
  DEFAULT_GOVERNANCE_RPC_ENDPOINT,
  DEFAULT_GOVERNANCE_WS_ENDPOINT,
  GovernanceBlockchainAdapter,
  GovernanceMockAdapter,
  GovernanceMockMaterializedProvider,
  GovernancePapiProvider,
  GovernanceRpcProvider,
  GovernanceUnavailableBlockchainProvider,
  GovernanceUnavailableMaterializedProvider,
  GOVERNANCE_QUERY_SURFACE_AVAILABILITY,
  GOVERNANCE_RUNTIME_WRITE_SURFACE,
} from "./adapters/governance";

// Stateful slices and workspace subsystems
export { portfolioStore, type KnownClientAsset, type TransferAssetKey } from "./portfolio/index.svelte";
export { governanceStore } from "./governance/index.svelte";
export { logStore } from "./log/index.svelte";
export { marketStore } from "./market/index.svelte";
export type {
  GovernancePanelProposal,
  GovernancePublicSubmissionOption,
  GovernanceViewerState,
} from "./governance/types";
export { layoutStore } from "./layout/index.svelte";
export { systemStore } from "./system/index.svelte";
export {
  walletStore,
  type WalletAccountOption,
  type WalletAccountSource,
  type WalletSignerStatus,
  type WalletState,
} from "./wallet/index.svelte";

// Shared
export * from "./shared/format";
export * from "./shared/read-model";
export * from "./shared/types";

// Layout subsystem
export { default as AppFooter } from "./layout/AppFooter.svelte";
export { default as AppHeader } from "./layout/AppHeader.svelte";
export { default as PaneHost } from "./layout/PaneHost.svelte";
export { default as SidebarPanel } from "./layout/SidebarPanel.svelte";
export { default as SplitHandle } from "./layout/SplitHandle.svelte";
export { default as TileContainer } from "./layout/TileContainer.svelte";
export { default as WorkspaceFrame } from "./layout/WorkspaceFrame.svelte";
export {
  CANONICAL_DEFAULT_LAYOUT_SPEC,
  LEGACY_SHIPPED_DEFAULT_LAYOUT_SPEC,
  buildLayoutFromSpec,
  matchesLayoutSpec,
  type LayoutLeafSpec,
  type LayoutNodeSpec,
  type LayoutSplitSpec,
} from "./layout/default-layout";

// Widgets
export * from "./widgets/";
