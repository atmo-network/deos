import assert from 'node:assert/strict';
import test from 'node:test';

import {
  clampAdjacentPrimarySize,
  collectDirectionalSegments,
  minimumTileHeight,
} from './split-groups.ts';
import { updateDirectionalSplitWeights } from './tree-ops.ts';
import type { PanelId, TileLeaf, TileNode, TileSplit } from './types.ts';

const PANE_MINIMUM = 96;
const HANDLE_EXTENT = 12;

function leaf(id: string, panel: PanelId = 'swap'): TileLeaf {
  return { type: 'leaf', id, tabs: [panel], activeTab: panel };
}

function threeRows(): TileSplit {
  return {
    type: 'split',
    id: 'root',
    direction: 'vertical',
    ratio: 0.4,
    children: [
      leaf('first'),
      {
        type: 'split',
        id: 'lower',
        direction: 'vertical',
        ratio: 0.5,
        children: [leaf('middle', 'log'), leaf('final', 'statistics')],
      },
    ],
  };
}

function segmentWeights(node: TileNode): Map<string, number> {
  if (node.type === 'leaf') {
    return new Map([[node.id, 1]]);
  }
  return new Map(
    collectDirectionalSegments(node, node.direction).map((segment) => [
      segment.node.id,
      segment.weight,
    ]),
  );
}

function assertWeight(
  weights: ReadonlyMap<string, number>,
  id: string,
  expected: number,
): void {
  assert.ok(Math.abs((weights.get(id) ?? -1) - expected) < 1e-12);
}

test('flattens consecutive same-axis splits into ordered weights', () => {
  const segments = collectDirectionalSegments(threeRows(), 'vertical');
  assert.deepEqual(
    segments.map((segment) => segment.node.id),
    ['first', 'middle', 'final'],
  );
  assertWeight(
    new Map(segments.map((item) => [item.node.id, item.weight])),
    'first',
    0.4,
  );
  assertWeight(
    new Map(segments.map((item) => [item.node.id, item.weight])),
    'middle',
    0.3,
  );
  assertWeight(
    new Map(segments.map((item) => [item.node.id, item.weight])),
    'final',
    0.3,
  );
});

test('reconstructs binary ratios from flattened rendered sizes', () => {
  const original = threeRows();
  const next = updateDirectionalSplitWeights(
    original,
    'root',
    new Map([
      ['first', 200],
      ['middle', 300],
      ['final', 500],
    ]),
  );
  const weights = segmentWeights(next);
  assertWeight(weights, 'first', 0.2);
  assertWeight(weights, 'middle', 0.3);
  assertWeight(weights, 'final', 0.5);
  assert.equal(original.ratio, 0.4, 'the source tree remains immutable');
});

test('first and final handles preserve every non-adjacent segment', () => {
  const original = threeRows();
  const firstResize = updateDirectionalSplitWeights(
    original,
    'root',
    new Map([
      ['first', 250],
      ['middle', 250],
      ['final', 500],
    ]),
  );
  assertWeight(segmentWeights(firstResize), 'final', 0.5);

  const finalResize = updateDirectionalSplitWeights(
    original,
    'root',
    new Map([
      ['first', 400],
      ['middle', 350],
      ['final', 250],
    ]),
  );
  assertWeight(segmentWeights(finalResize), 'first', 0.4);
});

test('recursive minima sum vertical panes and max horizontal siblings', () => {
  const vertical = threeRows();
  assert.equal(
    minimumTileHeight(vertical, PANE_MINIMUM, HANDLE_EXTENT),
    PANE_MINIMUM * 3 + HANDLE_EXTENT * 2,
  );
  const horizontal: TileSplit = {
    type: 'split',
    id: 'horizontal',
    direction: 'horizontal',
    ratio: 0.5,
    children: [vertical, leaf('side')],
  };
  assert.equal(
    minimumTileHeight(horizontal, PANE_MINIMUM, HANDLE_EXTENT),
    PANE_MINIMUM * 3 + HANDLE_EXTENT * 2,
  );
});

test('adjacent clamping preserves both minimums and pair extent', () => {
  const primary = clampAdjacentPrimarySize(300, 20, 96, 96);
  assert.equal(primary, 96);
  assert.equal(300 - primary, 204);

  const secondaryBound = clampAdjacentPrimarySize(300, 290, 96, 96);
  assert.equal(secondaryBound, 204);
  assert.equal(300 - secondaryBound, 96);
});

test('midpoint reset remains local and respects asymmetric minimums', () => {
  assert.equal(clampAdjacentPrimarySize(320, 160, 220, 96), 220);
  assert.equal(clampAdjacentPrimarySize(320, 160, 96, 220), 100);
});
