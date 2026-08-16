/*
Domain: Staking contracts
Owns: Native staking pool/account projection and operation-result shapes.
Excludes: Runtime staking logic, adapter transport implementation, and widget rendering.
Zone: Staking public contract; safe for adapters, stores, and widgets to import.
*/
export type NativeSecurityMode = 'TrustedSet' | 'LpBackedSelection';

export type NativeSecurityReadiness =
  | 'Inactive'
  | 'NativePoolMissing'
  | 'StakedAssetMissing'
  | 'LiquidityPoolMissing'
  | 'CanonicalLpMismatch'
  | 'EmptyNativeReserve'
  | 'EmptyStakedReserve'
  | 'EmptyLpIssuance'
  | 'ValuationUnavailable'
  | 'ParticipantIndexInconsistent'
  | 'EligibleOperatorSetEmpty'
  | 'CandidateSetInconsistent'
  | 'Ready';

export type NativeStakingPoolProjection = {
  nativeAssetId: number;
  stakedAssetId: number;
  lpAssetId: number;
  reserveNative: bigint;
  reserveStaked: bigint;
  lpTotalIssuance: bigint;
};

export type NativeLockedLpPositionProjection = {
  totalLockedLp: bigint;
  collatorLockedLp: bigint;
  governanceLockedLp: bigint;
  conservativeNativeValue: bigint | null;
};

export type NativeCollatorLpPositionProjection = {
  lpAssetId: number | null;
  lockedLp: bigint;
  pendingUnlockLp: bigint;
  pendingUnlockBlock: number | null;
  conservativeNativeValue: bigint | null;
};

export type NativeGovernanceCustodyPositionProjection = {
  lpAssetId: number | null;
  governanceLockedLp: bigint;
  pendingGovernanceLpUnlock: bigint;
  pendingGovernanceLpUnlockBlock: number | null;
  assetId: number;
  assetLocked: bigint;
  pendingAssetUnlock: bigint;
  pendingAssetUnlockBlock: number | null;
};

export type NativeSecurityBoundaryOutcome =
  | { type: 'NotReady'; readiness: NativeSecurityReadiness }
  | { type: 'SnapshotOpened' }
  | { type: 'SnapshotOpenFailed' };

export type NativeSecurityBoundaryDiagnosticProjection = {
  plannedEpoch: number;
  outcome: NativeSecurityBoundaryOutcome;
};

export type NativeStakingProjection = {
  isAvailable: boolean;
  accountAddress: string | null;
  securityMode: NativeSecurityMode | null;
  securityReadiness: NativeSecurityReadiness | null;
  securityEpoch: number | null;
  plannedSecurityEpoch: number | null;
  settlementObligationsRemain: boolean | null;
  boundaryDiagnostic: NativeSecurityBoundaryDiagnosticProjection | null;
  exchangeRate: bigint | null;
  pool: NativeStakingPoolProjection | null;
  accountPosition: NativeLockedLpPositionProjection | null;
};
