/*
Domain: Layout tree queries
Owns: Pure tile-tree inspection utilities for panel placement, active leaves, and capacity checks.
Excludes: Tree mutation, persistence, DOM rendering, and widget implementation.
Zone: Layout algorithm helper; depends only on layout contracts.
*/
import {
  MAX_TILE_LEAF_COUNT,
  type PanelId,
  type TileLeaf,
  type TileNode,
  isWorkspaceWidgetId,
} from './types';

export function findLeaf(node: TileNode, id: string): TileLeaf | null {
  if (node.type === 'leaf') {
    return node.id === id ? node : null;
  }
  return findLeaf(node.children[0], id) || findLeaf(node.children[1], id);
}

export function findLeafContainingPanel(
  node: TileNode,
  panelId: PanelId,
): TileLeaf | null {
  if (node.type === 'leaf') {
    return node.tabs.includes(panelId) ? node : null;
  }
  return (
    findLeafContainingPanel(node.children[0], panelId) ??
    findLeafContainingPanel(node.children[1], panelId)
  );
}

export function countLeaves(node: TileNode): number {
  if (node.type === 'leaf') {
    return 1;
  }
  return countLeaves(node.children[0]) + countLeaves(node.children[1]);
}

export function countPanels(node: TileNode): number {
  if (node.type === 'leaf') {
    return node.tabs.length;
  }
  return countPanels(node.children[0]) + countPanels(node.children[1]);
}

export function findFirstLeaf(node: TileNode): TileLeaf {
  return node.type === 'leaf' ? node : findFirstLeaf(node.children[0]);
}

function collectPanels(node: TileNode, out: PanelId[]): boolean {
  if (node.type === 'leaf') {
    if (
      node.tabs.length === 0 ||
      !node.tabs.includes(node.activeTab) ||
      node.tabs.some((panelId) => !isWorkspaceWidgetId(panelId))
    ) {
      return false;
    }
    out.push(...node.tabs);
    return true;
  }
  return (
    collectPanels(node.children[0], out) && collectPanels(node.children[1], out)
  );
}

export function isValidTree(node: TileNode): boolean {
  const panels: PanelId[] = [];
  if (!collectPanels(node, panels)) {
    return false;
  }
  return (
    countLeaves(node) <= MAX_TILE_LEAF_COUNT &&
    panels.length > 0 &&
    new Set(panels).size === panels.length
  );
}
