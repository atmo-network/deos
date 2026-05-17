/*
Domain: Layout tree ids
Owns: Local tile id generation, reset, and sequence recalculation from existing trees.
Excludes: Persistence keys, DOM ids outside tile nodes, and widget/panel identity.
Zone: Layout algorithm helper; depends only on layout tree contracts.
*/
import type { TileNode } from './types';

let nextId = 0;

export function genTileId(): string {
  return `t${nextId++}`;
}

export function resetTileIdSequence(): void {
  nextId = 0;
}

export function recalcNextTileId(node: TileNode): void {
  const num = Number.parseInt(node.id.replace('t', ''), 10);
  if (!Number.isNaN(num) && num >= nextId) {
    nextId = num + 1;
  }
  if (node.type === 'split') {
    recalcNextTileId(node.children[0]);
    recalcNextTileId(node.children[1]);
  }
}
