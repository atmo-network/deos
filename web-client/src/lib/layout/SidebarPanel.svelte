<!--
Domain: Sidebar reserved lane
Owns: Sidebar shell, configured sidebar widget loading, close affordance, and mobile-aware panel framing.
Excludes: Sidebar widget internals, account/settings store ownership, and center-pane layout.
Zone: Layout edge-lane component; composes configured lane widgets and UI Kit controls.
-->
<script lang="ts">
  import { X } from '@lucide/svelte';
  import type { Component } from 'svelte';

  import { reservedLaneWidgetsFor } from '$lib/layout/types';
  import { IconButton } from '$lib/ui';

  type WidgetComponent = Component;

  type Props = {
    id?: string;
    mobile: boolean;
    onclose: () => void;
  };

  let { id, mobile, onclose }: Props = $props();
  let accountWidgetComponent = $state<WidgetComponent | null>(null);
  let settingsWidgetComponent = $state<WidgetComponent | null>(null);

  const widgetIds = $derived(reservedLaneWidgetsFor('sidebar', mobile));
  const AccountWidget = $derived(accountWidgetComponent);
  const SettingsWidget = $derived(settingsWidgetComponent);

  async function ensureSidebarWidgetsLoaded(): Promise<void> {
    if (widgetIds.includes('account-menu') && !accountWidgetComponent) {
      const module = await import('$lib/widgets/AccountWidget.svelte');
      accountWidgetComponent = module.default;
    }
    if (widgetIds.includes('settings') && !settingsWidgetComponent) {
      const module = await import('$lib/widgets/SettingsWidget.svelte');
      settingsWidgetComponent = module.default;
    }
  }

  $effect(() => {
    void ensureSidebarWidgetsLoaded();
  });
</script>

<aside
  {id}
  class="@container h-full w-full min-h-0 rounded-2xl border border-(--mono-border) bg-[linear-gradient(135deg,#ffffff_0%,#f2f8ec_46%,#edf6fa_100%)] shadow-[0_8px_24px_rgba(44,50,30,0.05)] p-3 min-w-0 flex flex-col"
>
  <div class="flex items-center justify-between gap-2 pb-3">
    <div>
      <div class="text-xs font-medium text-(--mono-text)">Sidebar</div>
      <div class="text-[10px] text-(--mono-muted)">Reserved edge lane</div>
    </div>
    <IconButton onclick={onclose} label="Close sidebar">
      <X size={14} />
    </IconButton>
  </div>
  <div
    class="grid flex-1 min-h-0 content-start gap-3 overflow-y-auto pr-1 overscroll-contain"
  >
    {#if widgetIds.includes('account-menu')}
      {#if AccountWidget}
        <AccountWidget />
      {/if}
    {/if}
    {#if widgetIds.includes('settings')}
      {#if SettingsWidget}
        <SettingsWidget />
      {/if}
    {/if}
  </div>
</aside>
