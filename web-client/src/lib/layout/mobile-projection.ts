/*
Domain: Mobile workspace projection
Owns: Pure desktop-tree-to-mobile ordering, mobile order normalization, and nullable single-expanded-panel resolution.
Excludes: DOM rendering, pointer/keyboard interaction, persistence writes, and desktop tree mutation.
Zone: Layout algorithm helper; depends only on layout contracts.
*/
import type { PanelId, TileNode } from './types';

export type MobilePanelProjection = {
  panelId: PanelId;
  sourceLeafId: string;
  sourceTabIndex: number;
  activeInSourceLeaf: boolean;
};

function collectLeafPanels(
  node: TileNode,
  projection: MobilePanelProjection[],
): void {
  if (node.type === 'leaf') {
    node.tabs.forEach((panelId, sourceTabIndex) => {
      projection.push({
        panelId,
        sourceLeafId: node.id,
        sourceTabIndex,
        activeInSourceLeaf: node.activeTab === panelId,
      });
    });
    return;
  }
  collectLeafPanels(node.children[0], projection);
  collectLeafPanels(node.children[1], projection);
}

export function projectMobilePanels(
  root: TileNode,
  preferredOrder: readonly PanelId[] = [],
): MobilePanelProjection[] {
  const treeOrder: MobilePanelProjection[] = [];
  collectLeafPanels(root, treeOrder);

  const byPanelId = new Map(
    treeOrder.map((entry) => [entry.panelId, entry] as const),
  );
  const ordered: MobilePanelProjection[] = [];
  const seen = new Set<PanelId>();

  for (const panelId of preferredOrder) {
    const entry = byPanelId.get(panelId);
    if (entry && !seen.has(panelId)) {
      ordered.push(entry);
      seen.add(panelId);
    }
  }
  for (const entry of treeOrder) {
    if (!seen.has(entry.panelId)) {
      ordered.push(entry);
      seen.add(entry.panelId);
    }
  }
  return ordered;
}

export function moveMobilePanel(
  order: readonly PanelId[],
  panelId: PanelId,
  targetIndex: number,
): PanelId[] {
  const currentIndex = order.indexOf(panelId);
  if (currentIndex < 0 || order.length <= 1) {
    return [...order];
  }
  const next = [...order];
  next.splice(currentIndex, 1);
  const boundedTarget = Math.max(0, Math.min(targetIndex, next.length));
  next.splice(boundedTarget, 0, panelId);
  return next;
}

export function resolveMobileExpandedPanel(
  projection: readonly MobilePanelProjection[],
  preferredPanelId: PanelId | null,
): PanelId | null {
  if (preferredPanelId === null) {
    return null;
  }
  if (projection.some((entry) => entry.panelId === preferredPanelId)) {
    return preferredPanelId;
  }
  return (
    projection.find((entry) => entry.activeInSourceLeaf)?.panelId ??
    projection[0]?.panelId ??
    null
  );
}

export function mobileOrderFromProjection(
  projection: readonly MobilePanelProjection[],
): PanelId[] {
  return projection.map((entry) => entry.panelId);
}
