<script lang="ts">
  import { reservedLaneWidgetsFor } from "$lib/layout/types";

  type StatusWidgetComponent = any;

  type Props = {
    mobile: boolean;
  };

  let { mobile }: Props = $props();
  let statusWidgetComponent = $state<StatusWidgetComponent | null>(null);

  const widgetIds = $derived(reservedLaneWidgetsFor("footer", mobile));
  const StatusWidget = $derived(statusWidgetComponent);

  async function ensureStatusWidgetLoaded(): Promise<void> {
    if (statusWidgetComponent !== null || !widgetIds.includes("status")) {
      return;
    }
    const module = await import("$lib/widgets/StatusWidget.svelte");
    statusWidgetComponent = module.default;
  }

  $effect(() => {
    void ensureStatusWidgetLoaded();
  });
</script>

<footer class="shrink-0 flex justify-center items-center m-3 mt-0">
  {#if widgetIds.includes("status")}
    <div
      class="rounded-2xl border border-(--mono-border) bg-[linear-gradient(135deg,#ffffff_0%,#f2f8ec_46%,#edf6fa_100%)] px-1 py-1 shadow-[0_8px_24px_rgba(44,50,30,0.05)] overflow-x-auto [scrollbar-width:none]"
    >
      {#if StatusWidget}
        <StatusWidget />
      {:else}
        <div class="h-6 w-48 rounded-xl bg-(--mono-bg) animate-pulse"></div>
      {/if}
    </div>
  {/if}
</footer>
