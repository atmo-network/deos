import assert from 'node:assert/strict';
import test from 'node:test';

import { normalizeFrameState } from './legacy-normalization.ts';
import {
  mobileOrderFromProjection,
  moveMobilePanel,
  projectMobilePanels,
  resolveMobileExpandedPanel,
} from './mobile-projection.ts';
import type { PanelId, TileNode } from './types.ts';

const tree: TileNode = {
  type: 'split',
  id: 'root',
  direction: 'horizontal',
  ratio: 0.5,
  children: [
    {
      type: 'split',
      id: 'left',
      direction: 'vertical',
      ratio: 0.7,
      children: [
        {
          type: 'leaf',
          id: 'trade',
          tabs: ['swap', 'wallet'],
          activeTab: 'swap',
        },
        {
          type: 'leaf',
          id: 'evidence',
          tabs: ['log', 'statistics'],
          activeTab: 'statistics',
        },
      ],
    },
    {
      type: 'leaf',
      id: 'system',
      tabs: ['chart', 'automation', 'governance', 'wiki'],
      activeTab: 'automation',
    },
  ],
};

test('projects every tab in depth-first leaf and within-leaf order', () => {
  const projection = projectMobilePanels(tree);

  assert.deepEqual(mobileOrderFromProjection(projection), [
    'swap',
    'wallet',
    'log',
    'statistics',
    'chart',
    'automation',
    'governance',
    'wiki',
  ]);
  assert.deepEqual(
    projection.map(({ sourceLeafId, sourceTabIndex }) => [
      sourceLeafId,
      sourceTabIndex,
    ]),
    [
      ['trade', 0],
      ['trade', 1],
      ['evidence', 0],
      ['evidence', 1],
      ['system', 0],
      ['system', 1],
      ['system', 2],
      ['system', 3],
    ],
  );
});

test('normalizes a partial or duplicate mobile order without changing tree metadata', () => {
  const before = structuredClone(tree);
  const projection = projectMobilePanels(tree, [
    'wiki',
    'swap',
    'wiki',
    'governance',
  ]);

  assert.deepEqual(mobileOrderFromProjection(projection), [
    'wiki',
    'swap',
    'governance',
    'wallet',
    'log',
    'statistics',
    'chart',
    'automation',
  ]);
  assert.equal(projection[0]?.sourceLeafId, 'system');
  assert.deepEqual(tree, before);
});

test('moves one mobile panel with bounded indices and leaves the source order immutable', () => {
  const order = mobileOrderFromProjection(projectMobilePanels(tree));

  assert.deepEqual(moveMobilePanel(order, 'wiki', 1), [
    'swap',
    'wiki',
    'wallet',
    'log',
    'statistics',
    'chart',
    'automation',
    'governance',
  ]);
  assert.deepEqual(moveMobilePanel(order, 'swap', 99), [
    'wallet',
    'log',
    'statistics',
    'chart',
    'automation',
    'governance',
    'wiki',
    'swap',
  ]);
  assert.deepEqual(order, [
    'swap',
    'wallet',
    'log',
    'statistics',
    'chart',
    'automation',
    'governance',
    'wiki',
  ]);
});

test('keeps one valid expanded task, preserves collapse, and repairs stale ids', () => {
  const projection = projectMobilePanels(tree);

  assert.equal(resolveMobileExpandedPanel(projection, 'wiki'), 'wiki');
  assert.equal(resolveMobileExpandedPanel(projection, null), null);
  assert.equal(
    resolveMobileExpandedPanel(projection, 'unknown' as PanelId),
    'swap',
  );
  assert.equal(resolveMobileExpandedPanel([], 'wiki'), null);
});

test('upgrades legacy frame state with non-destructive mobile defaults', () => {
  assert.deepEqual(normalizeFrameState({ sidebar: { open: true } }), {
    sidebar: {
      placementVersion: 1,
      open: true,
      widgetOrder: ['account-menu', 'settings'],
      expandedWidgetId: 'account-menu',
    },
    mobile: { panelOrder: [], expandedPanelId: null },
  });
});

test('normalizes persisted mobile state to unique known panels', () => {
  assert.deepEqual(
    normalizeFrameState({
      sidebar: {
        open: false,
        widgetOrder: ['settings', 'account-menu', 'settings', 'unknown'],
        expandedWidgetId: null,
      },
      mobile: {
        panelOrder: ['wiki', 'swap', 'wiki', 'unknown', 7],
        expandedPanelId: 'governance',
      },
    }),
    {
      sidebar: {
        placementVersion: 1,
        open: false,
        widgetOrder: ['settings', 'account-menu'],
        expandedWidgetId: null,
      },
      mobile: {
        panelOrder: ['wiki', 'swap'],
        expandedPanelId: 'governance',
      },
    },
  );
  assert.deepEqual(
    normalizeFrameState({
      sidebar: {
        open: false,
        widgetOrder: null,
        expandedWidgetId: 'unknown',
      },
      mobile: { panelOrder: null, expandedPanelId: 'unknown' },
    }),
    {
      sidebar: {
        placementVersion: 1,
        open: false,
        widgetOrder: ['account-menu', 'settings'],
        expandedWidgetId: 'account-menu',
      },
      mobile: { panelOrder: [], expandedPanelId: null },
    },
  );
});
