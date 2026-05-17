<!--
Domain: Account chip widget
Owns: Compact active-account carousel, balance summary, and reserved header-lane account affordance.
Excludes: Wallet account selection policy, portfolio refresh logic, and layout lane ownership.
Zone: Presentation widget; consumes wallet/portfolio state and UI Kit formatting helpers.
-->
<script lang="ts">
  import { ChevronLeft, ChevronRight } from '@lucide/svelte';

  import { portfolioStore } from '$lib/portfolio/index.svelte';
  import { Button } from '$lib/ui';
  import { fmt, toFloat } from '$lib/ui/format';
  import { walletStore } from '$lib/wallet/index.svelte';

  type Props = {
    edge: 'left' | 'right';
    open: boolean;
    controlsId: string;
    onToggle: () => void;
  };

  let { edge, open, controlsId, onToggle }: Props = $props();

  const nativeBalance = $derived(
    fmt(toFloat(portfolioStore.userBalance.native)),
  );
  const foreignBalance = $derived(
    fmt(toFloat(portfolioStore.userBalance.foreign)),
  );
  const nativeAsset = $derived(portfolioStore.findAsset('native'));
  const foreignAsset = $derived(portfolioStore.findAsset('foreign'));
  const nativeSymbol = $derived(nativeAsset.symbol);
  const foreignSymbol = $derived(foreignAsset.symbol);
  const foreignCanonical = $derived(foreignAsset.isCanonical);
  const walletLabel = $derived(walletStore.state.selectedLabel);
  const signerStatus = $derived(walletStore.state.signerStatus);
  const toggleLabel = $derived(
    open ? `Close ${edge} sidebar` : `Open ${edge} sidebar`,
  );
  const pointsLeft = $derived(edge === 'left' ? !open : open);
</script>

<Button
  variant="secondary"
  onclick={onToggle}
  class="flex min-w-0 items-center gap-3 rounded-xl border-(--mono-border) bg-white px-2 py-1.5 text-xs tabnum shadow-sm hover:border-(--mono-cyan)"
  aria-expanded={open}
  aria-controls={controlsId}
  aria-label={toggleLabel}
>
  <span class="min-w-0 text-(--mono-muted)">
    <span class="truncate font-semibold text-(--mono-text)">{walletLabel}</span>
    <span
      class="ml-1 rounded-full bg-(--mono-bg) px-2 py-0.5 text-[10px] uppercase tracking-wide"
    >
      {signerStatus}
    </span>
  </span>
  <span class="hidden text-(--mono-muted) lg:inline">
    <span class="font-semibold text-(--mono-text)">{foreignBalance}</span>
    {foreignSymbol}{#if !foreignCanonical}*{/if}
  </span>
  <span class="hidden text-(--mono-muted) lg:inline">
    <span class="font-semibold text-(--mono-text)">{nativeBalance}</span>
    {nativeSymbol}
  </span>
  {#if pointsLeft}
    <ChevronLeft size={14} class="shrink-0 text-(--mono-muted)" />
  {:else}
    <ChevronRight size={14} class="shrink-0 text-(--mono-muted)" />
  {/if}
</Button>
