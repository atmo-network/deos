<!--
Domain: Pane widget host
Owns: Lazy widget component loading, widget overflow detection, and pane-local chrome state.
Excludes: Widget implementation, layout tree mutation, and adapter/domain state.
Zone: Layout rendering component; bridges layout panel ids to widget loader output.
-->
<script lang="ts">
  import { onMount } from 'svelte';

  import type { PanelId } from '$lib/layout/types';
  import { loadWidgetComponent } from '$lib/layout/widget-loader';
  import type { WidgetComponent } from '$lib/layout/widget-loader';

  type Props = {
    panelId: PanelId;
    panelLabels: Record<PanelId, string>;
  };

  let { panelId, panelLabels }: Props = $props();
  let scrollHostEl = $state<HTMLDivElement | null>(null);
  let loadedWidget = $state<WidgetComponent | null>(null);
  let loadingWidget = $state(false);
  let loadError = $state<string | null>(null);
  let hasVerticalOverflow = $state(false);
  let overflowCheckFrame = 0;

  const LoadedWidget = $derived(loadedWidget);

  async function ensureWidgetLoaded(activePanelId: PanelId): Promise<void> {
    loadingWidget = true;
    loadError = null;
    try {
      const widget = await loadWidgetComponent(activePanelId);
      if (panelId === activePanelId) {
        loadedWidget = widget;
      }
    } catch (error) {
      if (panelId === activePanelId) {
        loadError =
          error instanceof Error ? error.message : 'Widget load failed';
      }
    } finally {
      if (panelId === activePanelId) {
        loadingWidget = false;
      }
    }
  }

  function updateVerticalOverflowState() {
    if (!scrollHostEl) {
      hasVerticalOverflow = false;
      return;
    }
    // Measure against the borderless viewport to avoid border-triggered scrollbar flicker
    const borderlessClientHeight =
      scrollHostEl.clientHeight + (hasVerticalOverflow ? 2 : 0);
    hasVerticalOverflow =
      scrollHostEl.scrollHeight > borderlessClientHeight + 1;
  }

  function queueVerticalOverflowCheck() {
    if (typeof window === 'undefined') {
      return;
    }
    if (overflowCheckFrame !== 0) {
      cancelAnimationFrame(overflowCheckFrame);
    }
    overflowCheckFrame = requestAnimationFrame(() => {
      overflowCheckFrame = 0;
      updateVerticalOverflowState();
    });
  }

  $effect(() => {
    void ensureWidgetLoaded(panelId);
  });

  $effect(() => {
    panelId;
    loadedWidget;
    loadingWidget;
    loadError;
    queueVerticalOverflowCheck();
  });

  onMount(() => {
    queueVerticalOverflowCheck();
    if (!scrollHostEl) {
      return;
    }
    const resizeObserver = new ResizeObserver(() => {
      queueVerticalOverflowCheck();
    });
    resizeObserver.observe(scrollHostEl);
    const mutationObserver = new MutationObserver(() => {
      queueVerticalOverflowCheck();
    });
    mutationObserver.observe(scrollHostEl, {
      subtree: true,
      childList: true,
      characterData: true,
    });
    return () => {
      if (overflowCheckFrame !== 0) {
        cancelAnimationFrame(overflowCheckFrame);
      }
      mutationObserver.disconnect();
      resizeObserver.disconnect();
    };
  });
</script>

<div class="p-1 pt-0">
  <div
    bind:this={scrollHostEl}
    class={[
      '@container grid h-full min-h-0 overflow-y-scroll overflow-x-hidden overscroll-contain rounded-xl border pl-1.5',
      hasVerticalOverflow ? 'border-(--mono-border)' : 'border-transparent',
    ]}
    style:grid-template-rows="minmax(0, 1fr)"
  >
    {#if LoadedWidget}
      <div class="h-full min-h-full">
        <LoadedWidget />
      </div>
    {:else if loadError}
      <div
        class="flex items-center justify-center px-4 text-xs text-(--mono-warn)"
      >
        {loadError}
      </div>
    {:else if loadingWidget}
      <div
        class="flex items-center justify-center px-4 text-xs text-(--mono-muted)"
      >
        Loading {panelLabels[panelId]}…
      </div>
    {/if}
  </div>
</div>
