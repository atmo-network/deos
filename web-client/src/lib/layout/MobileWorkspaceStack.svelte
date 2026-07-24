<!--
Domain: Mobile workspace stack
Owns: One-dimensional panel projection, nullable single-expanded disclosure rendering, mobile panel activation, and accessible mobile ordering.
Excludes: Desktop tile mutation, reserved lanes, and widget internals.
Zone: Mobile layout renderer; consumes frame-owned mobile state and PaneWidgetHost.
-->
<script lang="ts">
  import { ChevronDown, GripVertical } from '@lucide/svelte';
  import { onDestroy, onMount, tick } from 'svelte';
  import { flip } from 'svelte/animate';

  import { layoutStore } from '$lib/layout/index.svelte';
  import {
    projectMobilePanels,
    moveMobilePanel as reorderMobilePanels,
    resolveMobileExpandedPanel,
  } from '$lib/layout/mobile-projection';
  import { PANEL_LABELS, type PanelId, type TileNode } from '$lib/layout/types';
  import { Button, Icon } from '$lib/ui';

  import PaneWidgetHost from './PaneWidgetHost.svelte';
  import { PANEL_ICONS } from './widget-icons';

  type Props = {
    node: TileNode;
  };

  let { node }: Props = $props();
  let reorderAnnouncement = $state('');
  let draggedPanelId = $state<PanelId | null>(null);
  let previewOrder = $state<PanelId[] | null>(null);
  let dragOriginIndex = -1;
  let activePointerId: number | null = null;
  let reducedMotion = $state(false);
  let motionQuery: MediaQueryList | null = null;

  const projection = $derived(
    projectMobilePanels(node, layoutStore.frame.mobile.panelOrder),
  );
  const expandedPanelId = $derived(
    resolveMobileExpandedPanel(
      projection,
      layoutStore.frame.mobile.expandedPanelId,
    ),
  );
  const renderedProjection = $derived.by(() => {
    if (!previewOrder) {
      return projection;
    }
    const byPanelId = new Map(
      projection.map((entry) => [entry.panelId, entry] as const),
    );
    return previewOrder.flatMap((panelId) => {
      const entry = byPanelId.get(panelId);
      return entry ? [entry] : [];
    });
  });

  onMount(() => {
    motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)');
    const syncReducedMotion = () => {
      reducedMotion = motionQuery?.matches ?? false;
    };
    syncReducedMotion();
    motionQuery.addEventListener('change', syncReducedMotion);
    return () => motionQuery?.removeEventListener('change', syncReducedMotion);
  });

  onDestroy(() => {
    if (typeof document !== 'undefined') {
      document.body.style.removeProperty('user-select');
    }
  });

  function panelHeaderId(panelId: PanelId): string {
    return `mobile-panel-header-${panelId}`;
  }

  function panelContentId(panelId: PanelId): string {
    return `mobile-panel-content-${panelId}`;
  }

  function panelGripId(panelId: PanelId): string {
    return `mobile-panel-grip-${panelId}`;
  }

  function togglePanel(panelId: PanelId): void {
    layoutStore.toggleMobileExpandedPanel(panelId);
  }

  async function commitPanelMove(
    panelId: PanelId,
    targetIndex: number,
  ): Promise<boolean> {
    const boundedTarget = Math.max(
      0,
      Math.min(targetIndex, projection.length - 1),
    );
    const currentIndex = projection.findIndex(
      (entry) => entry.panelId === panelId,
    );
    if (currentIndex < 0 || currentIndex === boundedTarget) {
      return false;
    }
    layoutStore.moveMobilePanel(panelId, boundedTarget);
    reorderAnnouncement = `${PANEL_LABELS[panelId]} moved to position ${boundedTarget + 1} of ${projection.length}`;
    await tick();
    return true;
  }

  async function handleGripKeydown(
    event: KeyboardEvent,
    panelId: PanelId,
    index: number,
  ): Promise<void> {
    if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') {
      return;
    }
    event.preventDefault();
    const targetIndex = event.key === 'ArrowUp' ? index - 1 : index + 1;
    if (await commitPanelMove(panelId, targetIndex)) {
      document.getElementById(panelGripId(panelId))?.focus();
    }
  }

  function startPointerReorder(event: PointerEvent, panelId: PanelId): void {
    if (event.pointerType === 'mouse' && event.button !== 0) {
      return;
    }
    event.preventDefault();
    activePointerId = event.pointerId;
    draggedPanelId = panelId;
    previewOrder = projection.map((entry) => entry.panelId);
    dragOriginIndex = projection.findIndex(
      (entry) => entry.panelId === panelId,
    );
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    document.body.style.userSelect = 'none';
    reorderAnnouncement = `${PANEL_LABELS[panelId]} picked up for reordering`;
  }

  function continuePointerReorder(event: PointerEvent): void {
    if (activePointerId !== event.pointerId || !draggedPanelId) {
      return;
    }
    event.preventDefault();
    const targetSection = document
      .elementFromPoint(event.clientX, event.clientY)
      ?.closest<HTMLElement>('[data-mobile-panel-id]');
    const targetPanelId = targetSection?.dataset.mobilePanelId as
      | PanelId
      | undefined;
    const targetIndex = renderedProjection.findIndex(
      (entry) => entry.panelId === targetPanelId,
    );
    if (targetIndex < 0 || !previewOrder) {
      return;
    }
    const nextOrder = reorderMobilePanels(
      previewOrder,
      draggedPanelId,
      targetIndex,
    );
    if (nextOrder.some((panelId, index) => panelId !== previewOrder?.[index])) {
      previewOrder = nextOrder;
    }
  }

  async function finishPointerReorder(
    event: PointerEvent,
    commit: boolean,
  ): Promise<void> {
    if (activePointerId !== event.pointerId || !draggedPanelId) {
      return;
    }
    const panelId = draggedPanelId;
    const finalIndex = renderedProjection.findIndex(
      (entry) => entry.panelId === panelId,
    );
    const grip = document.getElementById(panelGripId(panelId));
    if (grip?.hasPointerCapture(event.pointerId)) {
      grip.releasePointerCapture(event.pointerId);
    }
    if (commit && finalIndex >= 0 && finalIndex !== dragOriginIndex) {
      layoutStore.moveMobilePanel(panelId, finalIndex);
    }
    activePointerId = null;
    draggedPanelId = null;
    previewOrder = null;
    dragOriginIndex = -1;
    document.body.style.removeProperty('user-select');
    reorderAnnouncement = commit
      ? `${PANEL_LABELS[panelId]} moved to position ${finalIndex + 1} of ${projection.length}`
      : `${PANEL_LABELS[panelId]} reorder cancelled`;
    await tick();
    document.getElementById(panelGripId(panelId))?.focus();
  }
</script>

<svelte:window
  onpointermove={continuePointerReorder}
  onpointerup={(event) => void finishPointerReorder(event, true)}
  onpointercancel={(event) => void finishPointerReorder(event, false)}
/>

<div
  class="flex h-full min-h-0 [scroll-snap-type:y_proximity] flex-col gap-2 overflow-y-auto overscroll-contain px-3 [scrollbar-gutter:stable_both-edges] [scroll-padding-top:calc(4.25rem+max(0.75rem,env(safe-area-inset-top)))] [scroll-padding-bottom:calc(2.75rem+max(0.75rem,env(safe-area-inset-bottom)))] pt-[calc(4.25rem+max(0.75rem,env(safe-area-inset-top)))] pb-[calc(2.75rem+max(0.75rem,env(safe-area-inset-bottom)))]"
  aria-label="Mobile workspace widgets"
>
  {#each renderedProjection as entry, index (entry.panelId)}
    {@const expanded = entry.panelId === expandedPanelId}
    <section
      data-mobile-panel-id={entry.panelId}
      data-drop-preview={draggedPanelId === entry.panelId ? 'true' : undefined}
      animate:flip={{ duration: reducedMotion ? 0 : 180 }}
      class={[
        'grid min-w-0 shrink-0 snap-start overflow-hidden rounded-2xl border border-(--mono-border) bg-white shadow-[0_2px_10px_rgba(44,50,30,0.06)] transition-[opacity,box-shadow] motion-reduce:transition-none',
        draggedPanelId === entry.panelId &&
          'z-10 opacity-90 ring-2 ring-(--mono-purple) ring-offset-2 shadow-[0_8px_24px_rgba(44,50,30,0.18)]',
      ]}
    >
      <div
        class={[
          'grid min-w-0 grid-cols-[2.75rem_minmax(0,1fr)]',
          expanded && 'bg-(--mono-bg)',
        ]}
      >
        <Button
          id={panelGripId(entry.panelId)}
          variant="ghost"
          class="flex h-11 w-11 touch-none cursor-grab items-center justify-center rounded-none border-r border-(--mono-border) px-0 py-0 text-(--mono-muted) active:cursor-grabbing"
          label={`Reorder ${PANEL_LABELS[entry.panelId]}, position ${index + 1} of ${renderedProjection.length}. Drag or use arrow keys`}
          aria-keyshortcuts="ArrowUp ArrowDown"
          onkeydown={(event) =>
            void handleGripKeydown(event, entry.panelId, index)}
          onpointerdown={(event) => startPointerReorder(event, entry.panelId)}
        >
          <Icon icon={GripVertical} size="sm" />
        </Button>
        <h2 class="min-w-0">
          <Button
            id={panelHeaderId(entry.panelId)}
            variant="ghost"
            class="grid min-h-11 w-full grid-cols-[minmax(0,1fr)_2.75rem] items-center rounded-none px-0 py-0 text-sm font-semibold text-(--mono-text) hover:bg-(--mono-bg)"
            aria-expanded={expanded}
            aria-controls={panelContentId(entry.panelId)}
            onclick={() => togglePanel(entry.panelId)}
          >
            <span
              class="inline-flex min-w-0 items-center justify-center gap-1.5 px-3"
            >
              <Icon icon={PANEL_ICONS[entry.panelId]} size="sm" />
              <span class="truncate">{PANEL_LABELS[entry.panelId]}</span>
            </span>
            <Icon
              icon={ChevronDown}
              size="sm"
              class={[
                'justify-self-center text-(--mono-muted) transition-transform duration-150 motion-reduce:transition-none',
                expanded && 'rotate-180',
              ]}
            />
          </Button>
        </h2>
      </div>

      {#if expanded}
        <div
          id={panelContentId(entry.panelId)}
          role="region"
          aria-labelledby={panelHeaderId(entry.panelId)}
          class="grid border-t border-(--mono-border)"
        >
          <PaneWidgetHost
            panelId={entry.panelId}
            panelLabels={PANEL_LABELS}
            intrinsicHeight
          />
        </div>
      {/if}
    </section>
  {/each}
  <p class="sr-only" aria-live="polite" aria-atomic="true">
    {reorderAnnouncement}
  </p>
</div>
