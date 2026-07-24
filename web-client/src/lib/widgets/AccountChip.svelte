<!--
Domain: Account chip widget
Owns: Selected-account identity and reserved header-lane sidebar disclosure affordance.
Excludes: Wallet account selection policy, balances, portfolio refresh logic, and layout lane ownership.
Zone: Presentation widget; consumes local wallet identity and UI Kit controls.
-->
<script lang="ts">
  import {
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    UserRound,
  } from '@lucide/svelte';

  import { Button, Icon } from '$lib/ui';
  import { walletStore } from '$lib/wallet/index.svelte';

  type Props = {
    open: boolean;
    controlsId: string;
    placement: 'right' | 'bottom';
    onToggle: () => void;
  };

  let { open, controlsId, placement, onToggle }: Props = $props();

  const walletLabel = $derived(walletStore.state.selectedLabel);
  const toggleLabel = $derived(open ? 'Close sidebar' : 'Open sidebar');
  const disclosureIcon = $derived(
    placement === 'bottom'
      ? open
        ? ChevronDown
        : ChevronUp
      : open
        ? ChevronRight
        : ChevronLeft,
  );
</script>

<Button
  variant="secondary"
  onclick={onToggle}
  class="flex min-w-0 items-center gap-2 rounded-xl border-(--mono-border) bg-white p-2 text-xs shadow-sm hover:border-(--mono-cyan)"
  aria-expanded={open}
  aria-controls={controlsId}
  aria-label={toggleLabel}
>
  <Icon icon={UserRound} size="sm" class="text-(--mono-muted)" />
  <span class="min-w-0 truncate font-semibold text-(--mono-text)">
    {walletLabel}
  </span>
  <Icon icon={disclosureIcon} size="sm" class="text-(--mono-muted)" />
</Button>
