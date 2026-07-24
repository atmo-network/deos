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
    intrinsicHeight?: boolean;
  };

  let { panelId, panelLabels, intrinsicHeight = false }: Props = $props();
  let scrollHostEl = $state<HTMLDivElement | null>(null);
  let loadedWidget = $state<WidgetComponent | null>(null);
  let loadingWidget = $state(false);
  let loadError = $state<string | null>(null);
  let hasOverflow = $state(false);
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

  function updateOverflowState() {
    if (!scrollHostEl) {
      hasOverflow = false;
      return;
    }
    // Measure against the borderless viewport to avoid border-triggered scrollbar flicker.
    const borderAdjustment = hasOverflow ? 2 : 0;
    const borderlessClientHeight = scrollHostEl.clientHeight + borderAdjustment;
    const borderlessClientWidth = scrollHostEl.clientWidth + borderAdjustment;
    hasOverflow =
      scrollHostEl.scrollHeight > borderlessClientHeight + 1 ||
      scrollHostEl.scrollWidth > borderlessClientWidth + 1;
  }

  function queueOverflowCheck() {
    if (typeof window === 'undefined') {
      return;
    }
    if (overflowCheckFrame !== 0) {
      cancelAnimationFrame(overflowCheckFrame);
    }
    overflowCheckFrame = requestAnimationFrame(() => {
      overflowCheckFrame = 0;
      updateOverflowState();
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
    queueOverflowCheck();
  });

  onMount(() => {
    queueOverflowCheck();
    if (!scrollHostEl) {
      return;
    }
    const resizeObserver = new ResizeObserver(() => {
      queueOverflowCheck();
    });
    resizeObserver.observe(scrollHostEl);
    const mutationObserver = new MutationObserver(() => {
      queueOverflowCheck();
    });
    mutationObserver.observe(scrollHostEl, {
      subtree: true,
      childList: true,
      characterData: true,
      attributes: true,
      attributeFilter: ['open'],
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
      '@container grid min-h-0 overflow-auto overscroll-contain rounded-xl border scrollbar-gutter-both',
      intrinsicHeight ? 'min-h-24 max-h-[min(68dvh,48rem)]' : 'h-full',
      hasOverflow ? 'border-(--mono-border)' : 'border-transparent',
    ]}
    style:grid-template-rows="minmax(0, 1fr)"
  >
    {#if LoadedWidget}
      <div
        class={[
          'widget-scale',
          intrinsicHeight ? 'min-h-24' : 'h-full min-h-full',
        ]}
      >
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

<style>
  .widget-scale {
    --widget-em: 0.9375rem;
    --spacing: calc(var(--widget-em) / 4);
    --text-3xs: calc(var(--widget-em) * 0.6);
    --text-2xs: calc(var(--widget-em) * 0.6667);
    --text-compact: calc(var(--widget-em) * 0.7333);
    --text-xs: calc(var(--widget-em) * 0.8);
    --text-sm: calc(var(--widget-em) * 0.9333);
    --text-base: var(--widget-em);
    --text-lg: calc(var(--widget-em) * 1.125);
    font-size: var(--widget-em);
  }

  @container (max-width: 448px) {
    .widget-scale {
      --widget-em: 0.875rem;
    }
  }

  @container (min-width: 896px) {
    .widget-scale {
      --widget-em: 1rem;
    }
  }
</style>
