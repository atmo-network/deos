import assert from 'node:assert/strict';
import test from 'node:test';

import { ALL_WORKSPACE_WIDGETS, type TileNode } from './types.ts';
import { reconcileWorkspacePlacement } from './widget-placement.ts';

function treeWidgets(node: TileNode): string[] {
  return node.type === 'leaf'
    ? [...node.tabs]
    : [...treeWidgets(node.children[0]), ...treeWidgets(node.children[1])];
}

const tree: TileNode = {
  type: 'split',
  id: 'root',
  direction: 'horizontal',
  ratio: 0.5,
  children: [
    { type: 'leaf', id: 'left', tabs: ['swap'], activeTab: 'swap' },
    {
      type: 'leaf',
      id: 'right',
      tabs: ['wallet', 'account-menu'],
      activeTab: 'wallet',
    },
  ],
};

test('creates one lossless widget partition and gives explicit sidebar placement priority', () => {
  const placement = reconcileWorkspacePlacement(tree, [
    'wallet',
    'settings',
    'wallet',
  ]);
  const inTree = treeWidgets(placement.root);
  const combined = [...inTree, ...placement.sidebarOrder];

  assert.deepEqual(inTree, ['swap', 'account-menu']);
  assert.deepEqual(new Set(combined), new Set(ALL_WORKSPACE_WIDGETS));
  assert.equal(combined.length, ALL_WORKSPACE_WIDGETS.length);
  assert.deepEqual(treeWidgets(tree), ['swap', 'wallet', 'account-menu']);
});

test('keeps one tile widget when persisted sidebar placement claims every widget', () => {
  const placement = reconcileWorkspacePlacement(tree, ALL_WORKSPACE_WIDGETS);
  assert.deepEqual(treeWidgets(placement.root), ['swap']);
  assert.equal(placement.sidebarOrder.includes('swap'), false);
  assert.equal(placement.sidebarOrder.length, ALL_WORKSPACE_WIDGETS.length - 1);
});

test('allows Account and Settings to remain tile-owned workspace widgets', () => {
  const accountTree: TileNode = {
    type: 'leaf',
    id: 'account-tile',
    tabs: ['account-menu', 'settings'],
    activeTab: 'account-menu',
  };
  const placement = reconcileWorkspacePlacement(accountTree, ['swap']);
  assert.deepEqual(treeWidgets(placement.root), ['account-menu', 'settings']);
  assert.equal(placement.sidebarOrder.includes('account-menu'), false);
  assert.equal(placement.sidebarOrder.includes('settings'), false);
});
