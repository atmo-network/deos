/*
Domain: Portfolio contracts
Owns: Asset presentation metadata and balance item shapes used by account portfolio views.
Excludes: Balance refresh logic, wallet signer state, market quotes, and rendering components.
Zone: Portfolio public contract; safe for adapters, stores, and widgets to import.
*/
export type AssetPresentation = {
  kind: 'Native' | 'Local' | 'Foreign';
  assetId: number | null;
  symbol: string;
  isCanonical: boolean;
};

export type TransferAssetKey = 'native' | 'foreign' | `asset:${number}`;

export type AssetBalanceProjection = {
  presentation: AssetPresentation;
  balance: bigint;
  isPrimaryRouteAsset: boolean;
};
