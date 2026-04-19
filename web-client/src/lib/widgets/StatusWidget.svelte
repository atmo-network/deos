<script lang="ts">
  import { fromClientBoundedProjection } from "$lib/shared/read-model";
  import { ReadModelBadge } from "$lib/shared/ui";
  import { systemStore } from "$lib/system/index.svelte";
  import { walletStore } from "$lib/wallet/index.svelte";

  const footerProvenance = fromClientBoundedProjection(
    true,
    "statusWidget.footerStrip <- system connection state + finalized snapshot + selected account session",
  ).provenance;

  const footerItems = $derived.by(() => [
    {
      label: "Connection",
      value: systemStore.connectionState?.status ?? "unconfigured",
    },
    {
      label: "Signer",
      value: walletStore.state.signerStatus,
    },
    {
      label: "Account",
      value: walletStore.state.selectedLabel,
    },
    {
      label: "Finalized block",
      value: systemStore.snapshot?.blockNumber?.toString() ?? "—",
    },
  ]);
</script>

<div
  class="flex min-w-max items-center gap-1.5 text-[10px] leading-none whitespace-nowrap"
>
  <ReadModelBadge provenance={footerProvenance} tone="subtle" />
  {#each footerItems as item}
    <div
      class="inline-flex items-center gap-1.5 rounded-full border border-(--mono-border) bg-white/90 px-2 py-0.75"
    >
      <div class="text-[9px] uppercase tracking-wider text-(--mono-muted)">
        {item.label}
      </div>
      <div class="max-w-28 truncate text-[11px] font-medium text-(--mono-text)">
        {item.value}
      </div>
    </div>
  {/each}
</div>
