<!--
Domain: Sidebar reserved lane
Owns: Sidebar widget accordion composition, configured widget loading, and frame-owned disclosure and ordering rendering.
Excludes: Sidebar placement/dismissal, widget internals, persisted mutation internals, and center-pane topology.
Zone: Layout edge-lane component; composes configured lane widgets through frame-owned sidebar state.
-->
<script lang="ts">
  import { ChevronDown, GripVertical } from '@lucide/svelte';
  import { onDestroy, onMount, tick } from 'svelte';
  import { flip } from 'svelte/animate';

  import { layoutStore } from '$lib/layout/index.svelte';
  import {
    normalizeSidebarWidgetOrder,
    moveSidebarWidget as reorderSidebarWidgets,
  } from '$lib/layout/sidebar-projection';
  import { type SidebarWidgetId, WIDGET_LABELS } from '$lib/layout/types';
  import {
    type WidgetComponent,
    loadWidgetComponent,
  } from '$lib/layout/widget-loader';
  import { Button, Icon } from '$lib/ui';

  import { SIDEBAR_WIDGET_ICONS } from './widget-icons';

  type Props = {
    id?: string;
    mobile: boolean;
    crossContainerDragEnabled?: boolean;
  };

  let { id, crossContainerDragEnabled = false }: Props = $props();
  let loadedWidgets = $state<Partial<Record<SidebarWidgetId, WidgetComponent>>>(
    {},
  );
  let reorderAnnouncement = $state('');
  let draggedWidgetId = $state<SidebarWidgetId | null>(null);
  let previewOrder = $state<SidebarWidgetId[] | null>(null);
  let dragOriginIndex = -1;
  let activePointerId: number | null = null;
  let reducedMotion = $state(false);
  let motionQuery: MediaQueryList | null = null;

  const widgetOrder = $derived(
    normalizeSidebarWidgetOrder(layoutStore.frame.sidebar.widgetOrder),
  );
  const renderedWidgetOrder = $derived(previewOrder ?? widgetOrder);
  const expandedWidgetId = $derived(
    widgetOrder.includes(layoutStore.frame.sidebar.expandedWidgetId!)
      ? layoutStore.frame.sidebar.expandedWidgetId
      : null,
  );
  const ExpandedWidget = $derived(
    expandedWidgetId ? (loadedWidgets[expandedWidgetId] ?? null) : null,
  );

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

  function widgetHeaderId(widgetId: SidebarWidgetId): string {
    return `sidebar-widget-header-${widgetId}`;
  }

  function widgetLabelId(widgetId: SidebarWidgetId): string {
    return `sidebar-widget-label-${widgetId}`;
  }

  function widgetContentId(widgetId: SidebarWidgetId): string {
    return `sidebar-widget-content-${widgetId}`;
  }

  function widgetReorderId(widgetId: SidebarWidgetId): string {
    return `sidebar-widget-reorder-${widgetId}`;
  }

  function toggleWidget(widgetId: SidebarWidgetId): void {
    layoutStore.toggleSidebarExpandedWidget(widgetId);
  }

  async function commitWidgetMove(
    widgetId: SidebarWidgetId,
    targetIndex: number,
  ): Promise<boolean> {
    const boundedTarget = Math.max(
      0,
      Math.min(targetIndex, widgetOrder.length - 1),
    );
    const currentIndex = widgetOrder.indexOf(widgetId);
    if (currentIndex < 0 || currentIndex === boundedTarget) {
      return false;
    }
    layoutStore.moveSidebarWidget(widgetId, boundedTarget);
    reorderAnnouncement = `${WIDGET_LABELS[widgetId]} moved to position ${boundedTarget + 1} of ${widgetOrder.length}`;
    await tick();
    return true;
  }

  async function handleReorderKeydown(
    event: KeyboardEvent,
    widgetId: SidebarWidgetId,
    index: number,
  ): Promise<void> {
    if (event.shiftKey && event.key === 'ArrowLeft') {
      event.preventDefault();
      if (layoutStore.moveSidebarWidgetToFirstTile(widgetId)) {
        reorderAnnouncement = `${WIDGET_LABELS[widgetId]} moved to tiles`;
        await tick();
        document
          .querySelector<HTMLElement>(`[data-tab-id="${widgetId}"]`)
          ?.focus();
      }
      return;
    }
    if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') {
      return;
    }
    event.preventDefault();
    const targetIndex = event.key === 'ArrowUp' ? index - 1 : index + 1;
    if (await commitWidgetMove(widgetId, targetIndex)) {
      document.getElementById(widgetReorderId(widgetId))?.focus();
    }
  }

  function startPointerReorder(
    event: PointerEvent,
    widgetId: SidebarWidgetId,
  ): void {
    if (event.pointerType === 'mouse' && event.button !== 0) {
      return;
    }
    if (event.pointerType === 'mouse' && crossContainerDragEnabled) {
      return;
    }
    event.preventDefault();
    activePointerId = event.pointerId;
    draggedWidgetId = widgetId;
    previewOrder = [...widgetOrder];
    dragOriginIndex = widgetOrder.indexOf(widgetId);
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    document.body.style.userSelect = 'none';
    reorderAnnouncement = `${WIDGET_LABELS[widgetId]} picked up for reordering`;
  }

  function continuePointerReorder(event: PointerEvent): void {
    if (activePointerId !== event.pointerId || !draggedWidgetId) {
      return;
    }
    event.preventDefault();
    const targetSection = document
      .elementFromPoint(event.clientX, event.clientY)
      ?.closest<HTMLElement>('[data-sidebar-widget-id]');
    const targetWidgetId = targetSection?.dataset.sidebarWidgetId as
      | SidebarWidgetId
      | undefined;
    const targetIndex = renderedWidgetOrder.indexOf(targetWidgetId!);
    if (targetIndex < 0 || !previewOrder) {
      return;
    }
    const nextOrder = reorderSidebarWidgets(
      previewOrder,
      draggedWidgetId,
      targetIndex,
    );
    if (
      nextOrder.some((widgetId, index) => widgetId !== previewOrder?.[index])
    ) {
      previewOrder = nextOrder;
    }
  }

  async function finishPointerReorder(
    event: PointerEvent,
    commit: boolean,
  ): Promise<void> {
    if (activePointerId !== event.pointerId || !draggedWidgetId) {
      return;
    }
    const widgetId = draggedWidgetId;
    const finalIndex = renderedWidgetOrder.indexOf(widgetId);
    const reorderTarget = document.getElementById(widgetReorderId(widgetId));
    if (reorderTarget?.hasPointerCapture(event.pointerId)) {
      reorderTarget.releasePointerCapture(event.pointerId);
    }
    if (commit && finalIndex >= 0 && finalIndex !== dragOriginIndex) {
      layoutStore.moveSidebarWidget(widgetId, finalIndex);
    }
    activePointerId = null;
    draggedWidgetId = null;
    previewOrder = null;
    dragOriginIndex = -1;
    document.body.style.removeProperty('user-select');
    reorderAnnouncement = commit
      ? `${WIDGET_LABELS[widgetId]} moved to position ${finalIndex + 1} of ${widgetOrder.length}`
      : `${WIDGET_LABELS[widgetId]} reorder cancelled`;
    await tick();
    document.getElementById(widgetReorderId(widgetId))?.focus();
  }

  function startNativeWidgetDrag(
    event: DragEvent,
    widgetId: SidebarWidgetId,
  ): void {
    if (!crossContainerDragEnabled || !event.dataTransfer) {
      event.preventDefault();
      return;
    }
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', widgetId);
    draggedWidgetId = widgetId;
    previewOrder = [...widgetOrder];
    layoutStore.startSidebarWidgetDrag(widgetId);
    reorderAnnouncement = `${WIDGET_LABELS[widgetId]} picked up for workspace placement`;
  }

  function previewNativeDrop(
    event: DragEvent,
    targetWidgetId: SidebarWidgetId | null,
  ): void {
    const draggedTab = layoutStore.dragTab;
    if (!crossContainerDragEnabled || !draggedTab) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = 'move';
    }
    draggedWidgetId = draggedTab.tabId;
    const order = (previewOrder ?? widgetOrder).filter(
      (widgetId) => widgetId !== draggedTab.tabId,
    );
    const targetIndex = targetWidgetId
      ? order.indexOf(targetWidgetId)
      : order.length;
    order.splice(
      targetIndex < 0 ? order.length : targetIndex,
      0,
      draggedTab.tabId,
    );
    previewOrder = order;
  }

  async function commitNativeDrop(event: DragEvent): Promise<void> {
    const widgetId = draggedWidgetId;
    if (!crossContainerDragEnabled || !widgetId) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    const targetIndex = renderedWidgetOrder.indexOf(widgetId);
    const committed = layoutStore.dropOnSidebar(
      targetIndex < 0 ? renderedWidgetOrder.length : targetIndex,
    );
    draggedWidgetId = null;
    previewOrder = null;
    if (committed) {
      reorderAnnouncement = `${WIDGET_LABELS[widgetId]} moved to sidebar`;
      await tick();
      document.getElementById(widgetReorderId(widgetId))?.focus();
    }
  }

  function finishNativeDrag(): void {
    draggedWidgetId = null;
    previewOrder = null;
    layoutStore.endDrag();
  }

  async function ensureSidebarWidgetLoaded(
    widgetId: SidebarWidgetId,
  ): Promise<void> {
    if (loadedWidgets[widgetId]) {
      return;
    }
    const component = await loadWidgetComponent(widgetId);
    loadedWidgets = { ...loadedWidgets, [widgetId]: component };
  }

  $effect(() => {
    if (expandedWidgetId) {
      void ensureSidebarWidgetLoaded(expandedWidgetId);
    }
  });

  $effect(() => {
    if (!layoutStore.dragTab && activePointerId === null) {
      draggedWidgetId = null;
      previewOrder = null;
    }
  });
</script>

<svelte:window
  onpointermove={continuePointerReorder}
  onpointerup={(event) => void finishPointerReorder(event, true)}
  onpointercancel={(event) => void finishPointerReorder(event, false)}
/>

<aside
  {id}
  aria-label="Sidebar widgets"
  ondragover={(event) => previewNativeDrop(event, null)}
  ondrop={(event) => void commitNativeDrop(event)}
  class="@container flex h-full w-full min-h-0 min-w-0 flex-col gap-2 overflow-y-auto overscroll-contain rounded-2xl bg-[linear-gradient(135deg,#ffffff_0%,#f2f8ec_46%,#edf6fa_100%)] p-3"
>
  {#if renderedWidgetOrder.length === 0}
    <div
      role="status"
      class="flex min-h-24 items-center justify-center rounded-xl border border-dashed border-(--mono-border)/35 bg-white/70 px-4 text-center text-xs text-(--mono-muted)"
    >
      No widgets in sidebar
    </div>
  {/if}
  {#each renderedWidgetOrder as widgetId, index (widgetId)}
    {@const expanded = widgetId === expandedWidgetId}
    <section
      role="group"
      aria-label={`${WIDGET_LABELS[widgetId]} sidebar widget`}
      data-sidebar-widget-id={widgetId}
      data-drop-preview={draggedWidgetId === widgetId ? 'true' : undefined}
      animate:flip={{ duration: reducedMotion ? 0 : 180 }}
      ondragover={(event) => previewNativeDrop(event, widgetId)}
      ondrop={(event) => void commitNativeDrop(event)}
      class={[
        'grid min-w-0 shrink-0 overflow-hidden rounded-xl border border-(--mono-border) bg-white shadow-[0_2px_10px_rgba(44,50,30,0.05)] transition-[opacity,box-shadow] motion-reduce:transition-none',
        draggedWidgetId === widgetId &&
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
          id={widgetReorderId(widgetId)}
          variant="ghost"
          draggable={crossContainerDragEnabled}
          ondragstart={(event) => startNativeWidgetDrag(event, widgetId)}
          ondragend={finishNativeDrag}
          class="flex h-11 w-11 touch-none cursor-grab items-center justify-center rounded-none border-r border-(--mono-border) px-0 py-0 text-(--mono-muted) active:cursor-grabbing"
          label={`Reorder ${WIDGET_LABELS[widgetId]}, position ${index + 1} of ${renderedWidgetOrder.length}. Drag or use arrow keys`}
          aria-keyshortcuts="ArrowUp ArrowDown Shift+ArrowLeft"
          onkeydown={(event) =>
            void handleReorderKeydown(event, widgetId, index)}
          onpointerdown={(event) => startPointerReorder(event, widgetId)}
        >
          <Icon icon={GripVertical} size="sm" />
        </Button>
        <div role="heading" aria-level="2" class="min-w-0">
          <Button
            id={widgetHeaderId(widgetId)}
            variant="ghost"
            class="grid min-h-11 w-full grid-cols-[minmax(0,1fr)_2.75rem] items-center rounded-none px-0 py-0 text-sm font-semibold text-(--mono-text) hover:bg-(--mono-bg)"
            label={WIDGET_LABELS[widgetId]}
            aria-expanded={expanded}
            aria-controls={widgetContentId(widgetId)}
            onclick={() => toggleWidget(widgetId)}
          >
            <span
              id={widgetLabelId(widgetId)}
              class="inline-flex min-w-0 items-center justify-center gap-1.5 px-3"
            >
              <Icon icon={SIDEBAR_WIDGET_ICONS[widgetId]} size="sm" />
              <span class="truncate">{WIDGET_LABELS[widgetId]}</span>
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
        </div>
      </div>

      {#if expanded}
        <div
          id={widgetContentId(widgetId)}
          role="region"
          aria-labelledby={widgetLabelId(widgetId)}
          class="min-h-0 border-t border-(--mono-border) p-3"
        >
          {#if ExpandedWidget}
            <ExpandedWidget />
          {/if}
        </div>
      {/if}
    </section>
  {/each}
  <p class="sr-only" aria-live="polite" aria-atomic="true">
    {reorderAnnouncement}
  </p>
</aside>
