import {
  ALL_PANELS,
  MAX_TILE_LEAF_COUNT,
  type PanelId,
  type TileLeaf,
  type TileNode,
} from "./types";

export function findLeaf(node: TileNode, id: string): TileLeaf | null {
  if (node.type === "leaf") {
    return node.id === id ? node : null;
  }
  return findLeaf(node.children[0], id) || findLeaf(node.children[1], id);
}

export function countLeaves(node: TileNode): number {
  if (node.type === "leaf") {
    return 1;
  }
  return countLeaves(node.children[0]) + countLeaves(node.children[1]);
}

function collectPanels(node: TileNode, out: Set<PanelId>) {
  if (node.type === "leaf") {
    for (const tab of node.tabs) {
      out.add(tab);
    }
    return;
  }
  collectPanels(node.children[0], out);
  collectPanels(node.children[1], out);
}

export function isValidTree(node: TileNode): boolean {
  const panels = new Set<PanelId>();
  collectPanels(node, panels);
  return (
    countLeaves(node) <= MAX_TILE_LEAF_COUNT &&
    ALL_PANELS.every((panel) => panels.has(panel)) &&
    panels.size === ALL_PANELS.length
  );
}
