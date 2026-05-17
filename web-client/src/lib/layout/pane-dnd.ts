/*
Domain: Pane drag/drop projection
Owns: Tab preview lists, drop-edge detection, insertion-index math, and projection labels/classes.
Excludes: Pointer event lifecycle, store mutation, widget rendering, and tile tree mutation.
Zone: Layout interaction helper; depends only on layout contracts.
*/
import type { DragTabState, DropEdge, PanelId } from './types';

export type PreviewTabItem =
  | {
      key: string;
      kind: 'tab';
      tabId: PanelId;
    }
  | {
      key: string;
      kind: 'projection';
      tabId: PanelId;
    };

export function buildPreviewTabs(
  tabs: PanelId[],
  activeDrag: DragTabState | null,
  leafId: string,
  insertIndex: number | null,
): PreviewTabItem[] {
  const baseTabs =
    activeDrag && activeDrag.sourceLeafId === leafId
      ? tabs.filter((tabId) => tabId !== activeDrag.tabId)
      : tabs;
  const items = baseTabs.map<PreviewTabItem>((tabId) => ({
    key: `tab-${tabId}`,
    kind: 'tab',
    tabId,
  }));
  if (!activeDrag || insertIndex === null) {
    return items;
  }
  const projectionIndex = Math.max(0, Math.min(insertIndex, items.length));
  items.splice(projectionIndex, 0, {
    key: `projection-${leafId}-${activeDrag.tabId}`,
    kind: 'projection',
    tabId: activeDrag.tabId,
  });
  return items;
}

export function detectDropEdge(
  event: DragEvent,
  options: {
    containerEl: HTMLDivElement | undefined;
    tabBarEl: HTMLDivElement | undefined;
    paneGripEl: HTMLElement | undefined;
    zoneSize: number;
  },
): DropEdge | null {
  const { containerEl, tabBarEl, paneGripEl, zoneSize } = options;
  if (!containerEl || !tabBarEl || !paneGripEl) {
    return null;
  }
  const rect = containerEl.getBoundingClientRect();
  const insetTop = Math.max(
    tabBarEl.offsetHeight ?? 24,
    paneGripEl.offsetHeight ?? 14,
  );
  const x = event.clientX - rect.left;
  const y = event.clientY - rect.top;
  if (y < insetTop) {
    return null;
  }
  const contentHeight = rect.height - insetTop;
  const yInContent = y - insetTop;
  const nearBottom = yInContent > contentHeight - zoneSize;
  const nearLeft = x < zoneSize;
  const nearRight = x > rect.width - zoneSize;
  if (+nearBottom + +nearLeft + +nearRight !== 1) {
    return null;
  }
  if (nearBottom) {
    return 'bottom';
  }
  if (nearLeft) {
    return 'left';
  }
  return 'right';
}

export function edgeProjectionShellClass(edge: DropEdge): string {
  if (edge === 'right') {
    return 'pointer-events-none absolute top-2 right-2 bottom-2 w-[46%]';
  }
  if (edge === 'bottom') {
    return 'pointer-events-none absolute right-2 bottom-2 left-2 h-[44%]';
  }
  return 'pointer-events-none absolute top-2 bottom-2 left-2 w-[46%]';
}

export function edgeProjectionPayloadLabel(
  dragTab: DragTabState | null,
  labels: Record<PanelId, string>,
): string | null {
  if (!dragTab) {
    return null;
  }
  return labels[dragTab.tabId];
}

export function computeTabInsertIndex(
  event: DragEvent,
  tabBarEl: HTMLDivElement | undefined,
  fallbackLength: number,
): number {
  if (!tabBarEl) {
    return fallbackLength;
  }
  const buttons = tabBarEl.querySelectorAll<HTMLElement>('[data-tab-id]');
  for (let index = 0; index < buttons.length; index += 1) {
    const rect = buttons[index].getBoundingClientRect();
    if (event.clientX < rect.left + rect.width / 2) {
      return index;
    }
  }
  return buttons.length;
}
