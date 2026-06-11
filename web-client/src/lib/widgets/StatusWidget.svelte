<!--
Domain: Status widget
Owns: Compact footer status presentation for chain connection and active account readiness.
Excludes: System connection lifecycle, wallet store ownership, and footer lane layout.
Zone: Presentation widget; consumes system/wallet state and read-model provenance badges.
-->
<script lang="ts">
  import { fromClientBoundedProjection } from '$lib/read-model';
  import { systemStore } from '$lib/system/index.svelte';
  import { ReadModelBadge } from '$lib/ui';
  import { walletStore } from '$lib/wallet/index.svelte';

  const footerProvenance = fromClientBoundedProjection(
    true,
    'statusWidget.footerStrip <- system connection state + finalized snapshot + selected account session',
  ).provenance;

  const footerItems = $derived.by(() => [
    {
      label: 'Connection',
      value: systemStore.connectionState?.status ?? 'unconfigured',
    },
    {
      label: 'Signer',
      value: walletStore.state.signerStatus,
    },
    {
      label: 'Account',
      value: walletStore.state.selectedLabel,
    },
    {
      label: 'Finalized block',
      value: systemStore.snapshot?.blockNumber?.toString() ?? '—',
    },
  ]);
</script>

<div
  class="flex max-w-[calc(100vw-2rem)] flex-wrap items-center justify-center gap-1.5 text-[10px] leading-none"
>
  <ReadModelBadge provenance={footerProvenance} tone="subtle" />
  {#each footerItems as item}
    <div
      class="inline-flex items-center gap-1.5 rounded-full border border-(--mono-border) bg-white/90 px-2 py-0.75"
    >
      <div
        class="shrink-0 text-[9px] uppercase tracking-wider text-(--mono-muted)"
      >
        {item.label}
      </div>
      <div class="max-w-24 truncate text-[11px] font-medium text-(--mono-text)">
        {item.value}
      </div>
    </div>
  {/each}
</div>
