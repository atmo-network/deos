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
  const match = /^t(\d+)$/.exec(node.id);
  const num = match ? Number(match[1]) : null;
  if (num !== null && Number.isSafeInteger(num) && num >= nextId) {
    nextId = num + 1;
  }
  if (node.type === 'split') {
    recalcNextTileId(node.children[0]);
    recalcNextTileId(node.children[1]);
  }
}
