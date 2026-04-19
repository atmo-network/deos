import type { TileNode } from "./types";

let nextId = 0;

export function genTileId(): string {
  return `t${nextId++}`;
}

export function resetTileIdSequence(): void {
  nextId = 0;
}

export function recalcNextTileId(node: TileNode): void {
  const num = Number.parseInt(node.id.replace("t", ""), 10);
  if (!Number.isNaN(num) && num >= nextId) {
    nextId = num + 1;
  }
  if (node.type === "split") {
    recalcNextTileId(node.children[0]);
    recalcNextTileId(node.children[1]);
  }
}
