/*
Domain: Workspace widget placement
Owns: Lossless normalization of the widget partition between the tile tree and sidebar order.
Excludes: Persistence IO, drag/drop interaction, rendering, and widget implementation.
Zone: Pure layout-domain helper consumed by frame loading and placement mutations.
*/
import {
  ALL_WORKSPACE_WIDGETS,
  type PanelId,
  type SidebarWidgetId,
  type TileNode,
  isWorkspaceWidgetId,
} from './types.ts';

export type WorkspacePlacement = {
  root: TileNode;
  sidebarOrder: SidebarWidgetId[];
};

function collectTreeWidgets(node: TileNode, out: PanelId[]): void {
  if (node.type === 'leaf') {
    out.push(...node.tabs);
    return;
  }
  collectTreeWidgets(node.children[0], out);
  collectTreeWidgets(node.children[1], out);
}

function removeSidebarWidgetsFromTree(
  node: TileNode,
  sidebarWidgets: ReadonlySet<SidebarWidgetId>,
): TileNode | null {
  if (node.type === 'leaf') {
    const tabs = node.tabs.filter(
      (panelId) => isWorkspaceWidgetId(panelId) && !sidebarWidgets.has(panelId),
    );
    if (tabs.length === 0) {
      return null;
    }
    return {
      ...node,
      tabs,
      activeTab: tabs.includes(node.activeTab) ? node.activeTab : tabs[0],
    };
  }
  const first = removeSidebarWidgetsFromTree(node.children[0], sidebarWidgets);
  const second = removeSidebarWidgetsFromTree(node.children[1], sidebarWidgets);
  if (!first) return second;
  if (!second) return first;
  return { ...node, children: [first, second] };
}

export function reconcileWorkspacePlacement(
  root: TileNode,
  preferredSidebarOrder: readonly SidebarWidgetId[],
): WorkspacePlacement {
  const sidebarOrder = Array.from(
    new Set(preferredSidebarOrder.filter(isWorkspaceWidgetId)),
  );
  const rootWidgets: PanelId[] = [];
  collectTreeWidgets(root, rootWidgets);

  if (rootWidgets.every((widgetId) => sidebarOrder.includes(widgetId))) {
    const retainedRootWidget = rootWidgets[0];
    const retainedIndex = sidebarOrder.indexOf(retainedRootWidget);
    if (retainedIndex >= 0) {
      sidebarOrder.splice(retainedIndex, 1);
    }
  }

  const sidebarSet = new Set(sidebarOrder);
  const normalizedRoot = removeSidebarWidgetsFromTree(root, sidebarSet) ?? root;
  const placedRootWidgets: PanelId[] = [];
  collectTreeWidgets(normalizedRoot, placedRootWidgets);
  const placed = new Set<PanelId>([...placedRootWidgets, ...sidebarOrder]);

  for (const widgetId of ALL_WORKSPACE_WIDGETS) {
    if (!placed.has(widgetId)) {
      sidebarOrder.push(widgetId);
      placed.add(widgetId);
    }
  }

  return { root: normalizedRoot, sidebarOrder };
}
