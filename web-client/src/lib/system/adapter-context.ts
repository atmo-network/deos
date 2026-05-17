import type { AdapterRuntimeContext } from '$lib/adapters/contract';
/*
Domain: Adapter runtime context
Owns: System-provided runtime dependencies injected into blockchain adapter construction.
Excludes: Adapter transport implementation, endpoint constants, wallet store policy, and domain stores.
Zone: System composition helper; keeps adapter constructors independent from concrete stores.
*/
import { getBlockchainEndpoint } from '$lib/system/endpoint';
import { walletStore } from '$lib/wallet/index.svelte';
import { DEFAULT_DEOS_DAPP_NAME } from '$lib/wallet/signer';

export function buildAdapterRuntimeContext(): AdapterRuntimeContext {
  return {
    getEndpoint: () => getBlockchainEndpoint(),
    getSelectedAddress: () => walletStore.selectedAddress.trim(),
    dappName: DEFAULT_DEOS_DAPP_NAME,
  };
}
