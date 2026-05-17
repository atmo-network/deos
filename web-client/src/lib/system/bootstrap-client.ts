/*
Domain: Client bootstrap
Owns: Startup sequencing for wallet, system adapter connection, and initial log/refresh side effects.
Excludes: Store implementation, adapter transport internals, and UI rendering.
Zone: System composition entrypoint; called by app shell startup only.
*/
import { logStore } from '$lib/log/index.svelte';
import { systemStore } from '$lib/system/index.svelte';
import { walletStore } from '$lib/wallet/index.svelte';

export async function bootstrapClientWorkspace(): Promise<void> {
  await walletStore.init();
  await systemStore.init();
  const connectionState = systemStore.connectionState;
  if (connectionState?.status === 'connected') {
    logStore.add('Connected to DEOS network', 'info', {
      blockNumber: systemStore.snapshot?.blockNumber ?? null,
    });
    return;
  }
  if (connectionState?.status === 'error') {
    logStore.add(
      connectionState.message ?? 'DEOS network bootstrap failed',
      'error',
      {
        blockNumber: connectionState.finalizedBlockNumber,
      },
    );
  }
}
