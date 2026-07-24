<!--
Domain: Split resize handle
Owns: Pointer-driven split resizing interaction for workspace tile splits.
Excludes: Tile tree topology changes, widget rendering, and persisted layout policy.
Zone: Layout interaction component; delegates ratio updates to layout store.
-->
<script lang="ts">
  import { Ellipsis, EllipsisVertical } from '@lucide/svelte';

  import { layoutStore } from '$lib/layout/index.svelte';
  import { suppressTabFlipForLayoutResize } from '$lib/layout/tab-flip';
  import { Icon } from '$lib/ui';

  import { clampAdjacentPrimarySize } from './split-groups';

  type Props = {
    splitId: string;
    direction: 'horizontal' | 'vertical';
    minPrimaryPx: number;
    minSecondaryPx: number;
  };

  let { splitId, direction, minPrimaryPx, minSecondaryPx }: Props = $props();
  let handleEl: HTMLDivElement;
  let dragging = $state(false);
  let dragStartPointer = 0;
  let primaryStartSize = 0;
  let secondaryStartSize = 0;
  let pendingPrimarySize: number | null = null;
  let previewFrame = 0;

  const gripIcon = $derived(
    direction === 'horizontal' ? EllipsisVertical : Ellipsis,
  );

  function segmentElements(): HTMLElement[] {
    const parent = handleEl.parentElement;
    if (!parent) {
      return [];
    }
    return Array.from(parent.children).filter(
      (child): child is HTMLElement =>
        child instanceof HTMLElement &&
        child.dataset.splitSegment !== undefined,
    );
  }

  function elementExtent(element: HTMLElement): number {
    const rect = element.getBoundingClientRect();
    return direction === 'horizontal' ? rect.width : rect.height;
  }

  function pointerPosition(event: PointerEvent): number {
    return direction === 'horizontal' ? event.clientX : event.clientY;
  }

  function adjacentElements(): [HTMLElement, HTMLElement] | null {
    const primary = handleEl.previousElementSibling;
    const secondary = handleEl.nextElementSibling;
    return primary instanceof HTMLElement && secondary instanceof HTMLElement
      ? [primary, secondary]
      : null;
  }

  function freezeSegments(): boolean {
    const segments = segmentElements();
    if (segments.length < 2) {
      return false;
    }
    const sizes = segments.map(elementExtent);
    for (const [index, segment] of segments.entries()) {
      segment.style.flexBasis = `${sizes[index]}px`;
      segment.style.flexGrow = '0';
      segment.style.flexShrink = '0';
    }
    return true;
  }

  function restoreSegmentFlex(): void {
    for (const segment of segmentElements()) {
      segment.style.flexBasis = '0px';
      segment.style.flexGrow = segment.dataset.splitWeight ?? '0';
      segment.style.flexShrink = '1';
    }
  }

  function requestedPrimarySize(event: PointerEvent): number {
    const pairExtent = primaryStartSize + secondaryStartSize;
    const requested =
      primaryStartSize + pointerPosition(event) - dragStartPointer;
    const primaryMinimum =
      direction === 'vertical' ? minPrimaryPx : pairExtent * 0.15;
    const secondaryMinimum =
      direction === 'vertical' ? minSecondaryPx : pairExtent * 0.15;
    return clampAdjacentPrimarySize(
      pairExtent,
      requested,
      primaryMinimum,
      secondaryMinimum,
    );
  }

  function applyPreview(primarySize: number): void {
    const adjacent = adjacentElements();
    if (!adjacent) {
      return;
    }
    const pairExtent = primaryStartSize + secondaryStartSize;
    adjacent[0].style.flexBasis = `${primarySize}px`;
    adjacent[1].style.flexBasis = `${pairExtent - primarySize}px`;
  }

  function queuePreview(primarySize: number): void {
    pendingPrimarySize = primarySize;
    if (previewFrame !== 0) {
      return;
    }
    previewFrame = requestAnimationFrame(() => {
      previewFrame = 0;
      if (pendingPrimarySize !== null) {
        applyPreview(pendingPrimarySize);
      }
    });
  }

  function currentSegmentWeights(): Map<string, number> {
    return new Map(
      segmentElements().flatMap((segment) => {
        const id = segment.dataset.splitSegment;
        return id ? [[id, elementExtent(segment)] as const] : [];
      }),
    );
  }

  function finishDrag(event: PointerEvent, commit: boolean): void {
    if (!dragging) {
      return;
    }
    if (previewFrame !== 0) {
      cancelAnimationFrame(previewFrame);
      previewFrame = 0;
    }
    if (commit) {
      applyPreview(requestedPrimarySize(event));
    }
    const weights = commit ? currentSegmentWeights() : null;
    dragging = false;
    pendingPrimarySize = null;
    if (handleEl.hasPointerCapture(event.pointerId)) {
      handleEl.releasePointerCapture(event.pointerId);
    }
    restoreSegmentFlex();
    if (weights) {
      suppressTabFlipForLayoutResize();
      layoutStore.resizeDirectionalSplit(splitId, weights);
    }
  }

  function onPointerDown(event: PointerEvent) {
    const adjacent = adjacentElements();
    if (!adjacent || !freezeSegments()) {
      return;
    }
    dragging = true;
    dragStartPointer = pointerPosition(event);
    primaryStartSize = elementExtent(adjacent[0]);
    secondaryStartSize = elementExtent(adjacent[1]);
    pendingPrimarySize = primaryStartSize;
    handleEl.setPointerCapture(event.pointerId);
    event.preventDefault();
  }

  function onPointerMove(event: PointerEvent) {
    if (dragging) {
      queuePreview(requestedPrimarySize(event));
    }
  }

  function onPointerUp(event: PointerEvent) {
    finishDrag(event, true);
  }

  function onPointerCancel(event: PointerEvent) {
    finishDrag(event, false);
  }

  function onDoubleClick() {
    const adjacent = adjacentElements();
    if (!adjacent) {
      return;
    }
    const weights = currentSegmentWeights();
    const pairExtent = elementExtent(adjacent[0]) + elementExtent(adjacent[1]);
    const primaryId = adjacent[0].dataset.splitSegment;
    const secondaryId = adjacent[1].dataset.splitSegment;
    if (!primaryId || !secondaryId) {
      return;
    }
    const primaryMinimum =
      direction === 'vertical' ? minPrimaryPx : pairExtent * 0.15;
    const secondaryMinimum =
      direction === 'vertical' ? minSecondaryPx : pairExtent * 0.15;
    const primarySize = clampAdjacentPrimarySize(
      pairExtent,
      pairExtent / 2,
      primaryMinimum,
      secondaryMinimum,
    );
    weights.set(primaryId, primarySize);
    weights.set(secondaryId, pairExtent - primarySize);
    suppressTabFlipForLayoutResize();
    layoutStore.resizeDirectionalSplit(splitId, weights);
  }
</script>

<div
  bind:this={handleEl}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointercancel={onPointerCancel}
  ondblclick={onDoubleClick}
  role="separator"
  aria-orientation={direction === 'horizontal' ? 'vertical' : 'horizontal'}
  class={[
    'group relative z-10 flex flex-none touch-none select-none items-center justify-center bg-transparent',
    direction === 'horizontal'
      ? 'h-full w-3 cursor-col-resize'
      : 'h-3 w-full cursor-row-resize',
  ]}
>
  <span
    class={[
      'pointer-events-none [@media(pointer:coarse)]:pointer-events-auto absolute flex touch-none select-none items-center justify-center text-(--mono-muted) opacity-0 transition-[color,opacity] duration-150 group-hover:opacity-100 group-hover:text-(--mono-text)',
      direction === 'horizontal'
        ? 'inset-y-0 left-1/2 w-11 -translate-x-1/2'
        : 'inset-x-0 top-1/2 h-11 -translate-y-1/2',
      dragging && 'opacity-100 text-(--mono-purple)',
    ]}
  >
    <Icon icon={gripIcon} size="lg" />
  </span>
</div>
