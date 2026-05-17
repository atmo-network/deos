<!--
Domain: Center pane host
Owns: Tile leaf/split rendering, tab strip coordination, drop overlays, and pane recursion.
Excludes: Widget business logic, layout store mutation policy, and reserved edge-lane rendering.
Zone: Layout rendering component; consumes layout store and pane helpers only.
-->
<script lang="ts">
  import PaneDropOverlay from '$lib/layout/PaneDropOverlay.svelte';
  import PaneTopChrome from '$lib/layout/PaneTopChrome.svelte';
  import PaneWidgetHost from '$lib/layout/PaneWidgetHost.svelte';
  import { layoutStore } from '$lib/layout/index.svelte';
  import {
    buildPreviewTabs,
    computeTabInsertIndex,
    detectDropEdge,
    edgeProjectionPayloadLabel,
    edgeProjectionShellClass,
  } from '$lib/layout/pane-dnd';
  import { createTabFlipController } from '$lib/layout/tab-flip';
  import type { DropEdge, PanelId, TileLeaf } from '$lib/layout/types';
  import { PANEL_LABELS } from '$lib/layout/types';

  type Props = {
    leaf: TileLeaf;
  };

  const FLIP_DURATION_MS = 720;
  const RESIZE_FLIP_SUPPRESSION_MS = 120;
  const ZONE_SIZE = 40;

  let { leaf }: Props = $props();
  let hoveredEdge = $state<DropEdge | null>(null);
  let paneMergeHovered = $state(false);
  let tabInsertIndex = $state<number | null>(null);
  let tabBarEl = $state<HTMLDivElement | null>(null);
  let paneGripEl = $state<HTMLButtonElement | null>(null);
  let containerEl = $state<HTMLDivElement | null>(null);

  const dragTab = $derived(layoutStore.dragTab);
  const dragLeaf = $derived(layoutStore.dragLeaf);
  const isDragging = $derived(dragTab !== null || dragLeaf !== null);
  const isPaneDragging = $derived(dragLeaf !== null);
  const isLiftedSourcePane = $derived(dragLeaf?.sourceLeafId === leaf.id);
  const canMergePaneHere = $derived(
    dragLeaf !== null && dragLeaf.sourceLeafId !== leaf.id,
  );
  const canDropEdge = $derived.by(() => {
    if (dragTab) {
      return !(dragTab.sourceLeafId === leaf.id && leaf.tabs.length <= 1);
    }
    if (dragLeaf) {
      return dragLeaf.sourceLeafId !== leaf.id;
    }
    return false;
  });
  const previewTabs = $derived.by(() =>
    buildPreviewTabs(leaf.tabs, dragTab, leaf.id, tabInsertIndex),
  );

  function isLiftedSourceTab(tabId: PanelId): boolean {
    return dragTab?.sourceLeafId === leaf.id && dragTab.tabId === tabId;
  }

  function clearLocalDragPreview(): void {
    hoveredEdge = null;
    paneMergeHovered = false;
    tabInsertIndex = null;
  }

  function overlayTopOffset(): number {
    return Math.max(
      tabBarEl?.offsetHeight ?? 24,
      paneGripEl?.offsetHeight ?? 14,
    );
  }

  function contentProjectionTopOffset(): number {
    return overlayTopOffset() + 4;
  }

  const tabFlipController = createTabFlipController({
    flipDurationMs: FLIP_DURATION_MS,
    suppressionMs: RESIZE_FLIP_SUPPRESSION_MS,
  });

  function onTabDragStart(event: DragEvent, tabId: PanelId) {
    if (!event.dataTransfer) {
      return;
    }
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', tabId);
    requestAnimationFrame(() => layoutStore.startDrag(tabId, leaf.id));
  }

  function onPaneDragStart(event: DragEvent) {
    if (!event.dataTransfer) {
      return;
    }
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('application/x-deos-pane', leaf.id);
    requestAnimationFrame(() => layoutStore.startPaneDrag(leaf.id));
  }

  function onAnyDragEnd() {
    layoutStore.endDrag();
    clearLocalDragPreview();
  }

  function onOverlayDragOver(event: DragEvent) {
    if (!canDropEdge) {
      return;
    }
    paneMergeHovered = false;
    const edge = detectDropEdge(event, {
      containerEl: containerEl ?? undefined,
      tabBarEl: tabBarEl ?? undefined,
      paneGripEl: paneGripEl ?? undefined,
      zoneSize: ZONE_SIZE,
    });
    hoveredEdge = edge;
    if (edge) {
      event.preventDefault();
      if (event.dataTransfer) {
        event.dataTransfer.dropEffect = 'move';
      }
    }
  }

  function onOverlayDragLeave() {
    hoveredEdge = null;
  }

  function onOverlayDrop(event: DragEvent) {
    const edge = detectDropEdge(event, {
      containerEl: containerEl ?? undefined,
      tabBarEl: tabBarEl ?? undefined,
      paneGripEl: paneGripEl ?? undefined,
      zoneSize: ZONE_SIZE,
    });
    clearLocalDragPreview();
    if (!edge) {
      return;
    }
    event.preventDefault();
    layoutStore.dropOnEdge(leaf.id, edge);
  }

  function onPanePlateDragOver(event: DragEvent) {
    if (!canMergePaneHere) {
      paneMergeHovered = false;
      return;
    }
    event.preventDefault();
    hoveredEdge = null;
    paneMergeHovered = true;
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = 'move';
    }
  }

  function onPanePlateDragLeave(event: DragEvent) {
    const nextTarget = event.relatedTarget;
    if (nextTarget instanceof Node && paneGripEl?.contains(nextTarget)) {
      return;
    }
    paneMergeHovered = false;
  }

  function onPanePlateDrop(event: DragEvent) {
    if (!canMergePaneHere) {
      paneMergeHovered = false;
      return;
    }
    event.preventDefault();
    clearLocalDragPreview();
    layoutStore.dropPaneOnPlate(leaf.id);
  }

  function onTabBarDragOver(event: DragEvent) {
    if (!dragTab) {
      return;
    }
    event.preventDefault();
    paneMergeHovered = false;
    hoveredEdge = null;
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = 'move';
    }
    tabInsertIndex = computeTabInsertIndex(
      event,
      tabBarEl ?? undefined,
      leaf.tabs.length,
    );
  }

  function onTabBarDragLeave(event: DragEvent) {
    const nextTarget = event.relatedTarget;
    if (nextTarget instanceof Node && tabBarEl?.contains(nextTarget)) {
      return;
    }
    tabInsertIndex = null;
  }

  function onTabBarDrop(event: DragEvent) {
    event.preventDefault();
    if (!dragTab) {
      tabInsertIndex = null;
      return;
    }
    const { tabId, sourceLeafId } = dragTab;
    const index =
      tabInsertIndex ??
      computeTabInsertIndex(event, tabBarEl ?? undefined, leaf.tabs.length);
    clearLocalDragPreview();
    if (sourceLeafId === leaf.id) {
      layoutStore.reorderTab(leaf.id, tabId, index);
      layoutStore.endDrag();
      return;
    }
    layoutStore.dropOnTabBar(leaf.id, index);
  }

  $effect(() => {
    if (!tabBarEl || !containerEl) {
      return;
    }
    return tabFlipController.observe(tabBarEl, containerEl);
  });
</script>

<div
  bind:this={containerEl}
  class="relative grid h-full w-full overflow-hidden rounded-2xl border border-(--mono-border) bg-white shadow-[0_2px_10px_rgba(44,50,30,0.06)]"
  style:grid-template-rows="auto minmax(0, 1fr)"
>
  <PaneTopChrome
    bind:tabBarEl
    bind:paneGripEl
    {leaf}
    {previewTabs}
    panelLabels={PANEL_LABELS}
    animateTabs={tabFlipController.animate}
    {isLiftedSourceTab}
    {canMergePaneHere}
    {isLiftedSourcePane}
    {isPaneDragging}
    {paneMergeHovered}
    {onTabDragStart}
    {onAnyDragEnd}
    onSelectTab={(tabId) => layoutStore.setActiveTab(leaf.id, tabId)}
    {onTabBarDragOver}
    {onTabBarDragLeave}
    {onTabBarDrop}
    {onPaneDragStart}
    {onPanePlateDragLeave}
    {onPanePlateDragOver}
    {onPanePlateDrop}
  />

  <PaneWidgetHost panelId={leaf.activeTab} panelLabels={PANEL_LABELS} />

  <PaneDropOverlay
    {canDropEdge}
    {hoveredEdge}
    {isDragging}
    {isPaneDragging}
    {paneMergeHovered}
    overlayTop={overlayTopOffset()}
    contentProjectionTop={contentProjectionTopOffset()}
    payloadLabel={edgeProjectionPayloadLabel(dragTab, PANEL_LABELS)}
    {onOverlayDragOver}
    {onOverlayDragLeave}
    {onOverlayDrop}
    {edgeProjectionShellClass}
  />
</div>
