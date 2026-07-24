/*
Domain: Directional split groups
Owns: Same-axis tile flattening, recursive height minima, and adjacent resize bounds.
Excludes: DOM pointer lifecycle, layout persistence, widget rendering, and tile-tree mutation.
Zone: Pure layout algorithm helper; depends only on layout contracts.
*/
import type { TileNode } from './types';

export type DirectionalSegment = {
  node: TileNode;
  weight: number;
};

export function minimumTileHeight(
  node: TileNode,
  minimumPaneHeight: number,
  handleExtent: number,
): number {
  if (node.type === 'leaf') {
    return minimumPaneHeight;
  }
  const first = minimumTileHeight(
    node.children[0],
    minimumPaneHeight,
    handleExtent,
  );
  const second = minimumTileHeight(
    node.children[1],
    minimumPaneHeight,
    handleExtent,
  );
  return node.direction === 'vertical'
    ? first + handleExtent + second
    : Math.max(first, second);
}

export function collectDirectionalSegments(
  node: TileNode,
  direction: 'horizontal' | 'vertical',
  weight = 1,
): DirectionalSegment[] {
  if (node.type === 'leaf' || node.direction !== direction) {
    return [{ node, weight }];
  }
  const ratio = Math.max(0, Math.min(1, node.ratio));
  return [
    ...collectDirectionalSegments(node.children[0], direction, weight * ratio),
    ...collectDirectionalSegments(
      node.children[1],
      direction,
      weight * (1 - ratio),
    ),
  ];
}

export function clampAdjacentPrimarySize(
  pairExtent: number,
  requested: number,
  primaryMinimum: number,
  secondaryMinimum: number,
): number {
  if (pairExtent < primaryMinimum + secondaryMinimum) {
    return Math.max(0, Math.min(pairExtent, requested));
  }
  return Math.max(
    primaryMinimum,
    Math.min(pairExtent - secondaryMinimum, requested),
  );
}
