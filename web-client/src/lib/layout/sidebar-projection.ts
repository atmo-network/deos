/*
Domain: Sidebar widget projection
Owns: Canonical sidebar widget ordering, nullable expansion resolution, and bounded reorder transforms.
Excludes: Frame persistence, sidebar rendering, widget loading, and pointer interaction.
Zone: Pure layout-domain helper consumed by frame normalization, store mutations, and sidebar composition.
*/
import { type SidebarWidgetId, isWorkspaceWidgetId } from './types.ts';

export const isSidebarWidgetId = isWorkspaceWidgetId;

export function normalizeSidebarWidgetOrder(
  preferredOrder: readonly SidebarWidgetId[] = [],
): SidebarWidgetId[] {
  const preferred = Array.from(
    new Set(preferredOrder.filter(isSidebarWidgetId)),
  );
  return preferred;
}

export function insertSidebarWidget(
  sourceOrder: readonly SidebarWidgetId[],
  widgetId: SidebarWidgetId,
  targetIndex: number,
): SidebarWidgetId[] {
  const order = normalizeSidebarWidgetOrder(sourceOrder).filter(
    (candidate) => candidate !== widgetId,
  );
  const boundedTarget = Math.max(0, Math.min(targetIndex, order.length));
  order.splice(boundedTarget, 0, widgetId);
  return order;
}

export function moveSidebarWidget(
  sourceOrder: readonly SidebarWidgetId[],
  widgetId: SidebarWidgetId,
  targetIndex: number,
): SidebarWidgetId[] {
  const order = normalizeSidebarWidgetOrder(sourceOrder);
  const currentIndex = order.indexOf(widgetId);
  if (currentIndex < 0) {
    return order;
  }
  return insertSidebarWidget(order, widgetId, targetIndex);
}

export function resolveSidebarExpandedWidget(
  order: readonly SidebarWidgetId[],
  expandedWidgetId: SidebarWidgetId | null,
): SidebarWidgetId | null {
  if (expandedWidgetId === null) {
    return null;
  }
  const normalizedOrder = normalizeSidebarWidgetOrder(order);
  return normalizedOrder.includes(expandedWidgetId)
    ? expandedWidgetId
    : (normalizedOrder[0] ?? null);
}
