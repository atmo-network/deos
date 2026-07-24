<!--
Domain: Status widget
Owns: Compact footer status presentation for chain connection and active account readiness.
Excludes: System connection lifecycle, wallet store ownership, and footer lane layout.
Zone: Presentation widget; consumes system/wallet state and UI Kit helpers.
-->
<script lang="ts">
  import { Blocks, KeyRound, UserRound, Wifi, WifiOff } from '@lucide/svelte';

  import { resolveChainSurfaceState } from '$lib/system/connection-surface';
  import { systemStore } from '$lib/system/index.svelte';
  import { Icon, Tooltip } from '$lib/ui';
  import { walletStore } from '$lib/wallet/index.svelte';

  const connectionStatus = $derived(
    systemStore.connectionState?.status ?? 'initializing',
  );
  const chainSurface = $derived(
    resolveChainSurfaceState(
      systemStore.connectionState,
      systemStore.snapshot !== null,
    ),
  );
  const connectionMessage = $derived(
    systemStore.connectionState?.status === 'connected'
      ? chainSurface.detail
      : (systemStore.connectionState?.message ?? chainSurface.detail),
  );
  const finalizedBlockValue = $derived.by(() => {
    const blockNumber = systemStore.snapshot?.blockNumber;
    if (blockNumber === null || blockNumber === undefined) {
      return '—';
    }
    if (chainSurface.status === 'stale') {
      return `${blockNumber} · stale`;
    }
    if (chainSurface.status === 'preview') {
      return `${blockNumber} · preview`;
    }
    return blockNumber.toString();
  });
  const finalizedBlockTone = $derived(
    chainSurface.status === 'ready'
      ? 'text-(--mono-cyan)'
      : chainSurface.status === 'stale' || chainSurface.status === 'preview'
        ? 'text-(--mono-orange)'
        : chainSurface.status === 'error'
          ? 'text-(--mono-pink)'
          : 'text-(--mono-muted)',
  );

  const statusItems = $derived.by(() => [
    {
      key: 'network',
      label: 'Network',
      value: connectionStatus,
      detail: connectionMessage,
      icon:
        systemStore.connectionState?.status === 'connected' ? Wifi : WifiOff,
      tone:
        systemStore.connectionState?.status === 'connected'
          ? 'text-(--mono-green)'
          : connectionStatus === 'error'
            ? 'text-(--mono-pink)'
            : 'text-(--mono-muted)',
    },
    {
      key: 'signer',
      label: 'Signer',
      value: walletStore.state.signerStatus,
      detail: walletStore.state.signerMessage,
      icon: KeyRound,
      tone:
        walletStore.state.signerStatus === 'available'
          ? 'text-(--mono-green)'
          : 'text-(--mono-muted)',
    },
    {
      key: 'account',
      label: 'Account',
      value: walletStore.state.selectedLabel,
      detail: walletStore.state.selectedAddress || 'No account selected',
      icon: UserRound,
      tone: 'text-(--mono-purple)',
    },
    {
      key: 'block',
      label: 'Finalized block',
      value: finalizedBlockValue,
      detail:
        chainSurface.status === 'ready'
          ? 'Latest bounded finalized-chain snapshot from the live provider'
          : chainSurface.detail,
      icon: Blocks,
      tone: finalizedBlockTone,
    },
  ]);
</script>

<div
  class="status-container flex min-h-6 w-max max-w-full flex-nowrap items-center justify-center gap-1.5 text-2xs leading-tight"
  aria-label="Workspace status"
>
  {#each statusItems as item}
    <Tooltip
      aria-label={`${item.label}: ${item.value}. ${item.detail}`}
      class="inline-flex min-h-6 min-w-0 flex-none cursor-help items-center justify-center gap-1.5 rounded-lg bg-transparent px-2 py-0.5 text-(--mono-text) max-[28rem]:basis-8 max-[28rem]:px-2"
      contentClass="max-w-80"
    >
      {#snippet content()}
        <div class="font-semibold">{item.label}: {item.value}</div>
        <div class="mt-1 break-all text-(--mono-muted)">{item.detail}</div>
      {/snippet}
      <Icon icon={item.icon} size="sm" class={item.tone} />
      <span
        class="shrink-0 text-3xs uppercase tracking-wider text-(--mono-muted) max-[44rem]:hidden"
      >
        {item.label}
      </span>
      <span
        class="min-w-0 truncate text-compact font-medium max-[28rem]:hidden"
      >
        {item.value}
      </span>
    </Tooltip>
  {/each}
</div>
