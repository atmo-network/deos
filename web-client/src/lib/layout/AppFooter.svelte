<!--
Domain: App footer lane
Owns: Reserved bottom lane composition and status widget placement.
Excludes: Status widget internals, center-pane rendering, and layout mutation.
Zone: Layout edge-lane component; composes configured footer widgets only.
-->
<script lang="ts">
  import type { Component } from 'svelte';

  import { reservedLaneWidgetsFor } from '$lib/layout/types';

  type StatusWidgetComponent = Component;

  type Props = {
    mobile: boolean;
  };

  let { mobile }: Props = $props();
  let statusWidgetComponent = $state<StatusWidgetComponent | null>(null);

  const widgetIds = $derived(reservedLaneWidgetsFor('footer', mobile));
  const StatusWidget = $derived(statusWidgetComponent);

  async function ensureStatusWidgetLoaded(): Promise<void> {
    if (statusWidgetComponent !== null || !widgetIds.includes('status')) {
      return;
    }
    const module = await import('$lib/widgets/StatusWidget.svelte');
    statusWidgetComponent = module.default;
  }

  $effect(() => {
    void ensureStatusWidgetLoaded();
  });
</script>

<footer
  class={[
    'shrink-0 flex justify-center items-center mx-3 mt-0',
    mobile ? 'mb-[max(0.75rem,env(safe-area-inset-bottom))]' : 'mb-3',
  ]}
>
  {#if widgetIds.includes('status')}
    <div
      class="min-h-8 w-max max-w-[calc(100vw-1.5rem)] rounded-xl border border-(--mono-border) bg-white px-1 py-0.5 shadow-[0_8px_24px_rgba(44,50,30,0.05)] overflow-x-auto scrollbar-none"
    >
      {#if StatusWidget}
        <StatusWidget />
      {:else}
        <div class="h-6 w-48 rounded-xl bg-(--mono-bg) animate-pulse"></div>
      {/if}
    </div>
  {/if}
</footer>
