/*
Domain: Runtime asset helpers
Owns: DEOS runtime asset enum construction, classification, keying, symbols, and native-balance extraction.
Excludes: Asset registry storage reads, portfolio store state, and UI formatting.
Zone: Transport/runtime helper; depends on PAPI enum shape only.
*/
import { Enum as PapiEnum } from 'polkadot-api';

export const TYPE_FOREIGN = 0xf000_0000;

export type RuntimeAssetKind =
  | { type: 'Native'; value: undefined }
  | { type: 'Local'; value: number }
  | { type: 'Foreign'; value: number };
export type RuntimeAssetWithId = { type: 'Local' | 'Foreign'; value: number };

export const NATIVE_ASSET: RuntimeAssetKind = PapiEnum<
  RuntimeAssetKind,
  'Native'
>('Native');

export function localAsset(assetId: number): RuntimeAssetKind {
  return PapiEnum<RuntimeAssetKind, 'Local'>('Local', assetId);
}

export function foreignAsset(assetId: number): RuntimeAssetKind {
  return PapiEnum<RuntimeAssetKind, 'Foreign'>('Foreign', assetId);
}

export function accountIdRecipient(accountId: string) {
  return PapiEnum('Id', accountId);
}

export function isForeignAssetKind(
  asset: RuntimeAssetKind,
): asset is { type: 'Foreign'; value: number } {
  return asset.type === 'Foreign';
}

export function isAssetWithId(
  asset: RuntimeAssetKind,
): asset is RuntimeAssetWithId {
  return asset.type === 'Local' || asset.type === 'Foreign';
}

export function runtimeAssetKey(asset: RuntimeAssetKind): string {
  return isAssetWithId(asset) ? `${asset.type}:${asset.value}` : asset.type;
}

export function dedupeRuntimeAssets(
  assets: RuntimeAssetKind[],
): RuntimeAssetKind[] {
  const deduped: RuntimeAssetKind[] = [];
  const seen = new Set<string>();
  for (const asset of assets) {
    const key = runtimeAssetKey(asset);
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    deduped.push(asset);
  }
  return deduped;
}

export function nativeFreeBalance(
  account: { data?: { free?: bigint } } | null | undefined,
): bigint {
  return account?.data?.free ?? 0n;
}

export function decodeBytes(bytes: Uint8Array | undefined): string | null {
  if (!bytes || bytes.length === 0) {
    return null;
  }
  const decoded = new TextDecoder().decode(bytes).replace(/\0/g, '').trim();
  return decoded.length > 0 ? decoded : null;
}

export function fallbackAssetSymbol(asset: RuntimeAssetKind): string {
  switch (asset.type) {
    case 'Native':
      return 'NTVE';
    case 'Local':
      return `LOCAL-${asset.value}`;
    case 'Foreign':
      return `FOREIGN-${asset.value.toString(16).toUpperCase()}`;
  }
}
