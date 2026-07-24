<!--
Domain: App header lane
Owns: Reserved top lane composition, layout reset affordance, and account-chip placement.
Excludes: Account widget internals, layout store policy, and center-pane rendering.
Zone: Layout edge-lane component; may compose configured lane widgets and UI Kit controls.
-->
<script lang="ts">
  import { Sparkle } from '@lucide/svelte';
  import type { Component } from 'svelte';

  import { reservedLaneWidgetsFor } from '$lib/layout/types';
  import { Icon } from '$lib/ui';

  type AccountChipComponent = Component<{
    controlsId: string;
    open: boolean;
    placement: 'right' | 'bottom';
    onToggle: () => void;
  }>;

  type Props = {
    mobile: boolean;
    sidebarId: string;
    sidebarOpen: boolean;
    sidebarPlacement: 'right' | 'bottom';
    onToggleSidebar: () => void;
  };

  let {
    mobile,
    sidebarId,
    sidebarOpen,
    sidebarPlacement,
    onToggleSidebar,
  }: Props = $props();
  let accountChipComponent = $state<AccountChipComponent | null>(null);

  const widgetIds = $derived(reservedLaneWidgetsFor('header', mobile));
  const AccountChip = $derived(accountChipComponent);

  async function ensureAccountChipLoaded(): Promise<void> {
    if (accountChipComponent !== null || !widgetIds.includes('account-chip')) {
      return;
    }
    const module = await import('$lib/widgets/AccountChip.svelte');
    accountChipComponent = module.default;
  }

  $effect(() => {
    void ensureAccountChipLoaded();
  });
</script>

<header
  class={[
    'shrink-0 mx-3 mb-0',
    mobile ? 'mt-[max(0.75rem,env(safe-area-inset-top))]' : 'mt-3',
  ]}
>
  <div class="grid gap-2">
    <div class="flex items-center justify-between gap-3 h-14">
      <div
        class="flex h-full items-center gap-3 rounded-2xl border border-(--mono-border) bg-white px-3"
      >
        <h1
          class="inline-flex items-center gap-2 rounded-full text-md font-medium text-(--mono-border)"
        >
          <Icon
            class="fill-white text-(--mono-border)"
            icon={Sparkle}
            size="lg"
          />
          DEOS
        </h1>
      </div>

      <div
        class="flex h-full items-center gap-2 rounded-2xl border border-(--mono-border) bg-[linear-gradient(135deg,#ffffff_0%,#f2f8ec_46%,#edf6fa_100%)] px-2 shadow-[0_8px_32px_rgba(44,50,30,0.06)]"
      >
        {#if widgetIds.includes('account-chip')}
          {#if AccountChip}
            <AccountChip
              controlsId={sidebarId}
              open={sidebarOpen}
              placement={sidebarPlacement}
              onToggle={onToggleSidebar}
            />
          {:else}
            <div
              class="h-8 w-24 rounded-xl border border-(--mono-border)/40 bg-(--mono-bg) animate-pulse"
            ></div>
          {/if}
        {/if}
      </div>
    </div>
  </div>
</header>
