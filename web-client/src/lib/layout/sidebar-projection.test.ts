import assert from 'node:assert/strict';
import test from 'node:test';

import {
  insertSidebarWidget,
  moveSidebarWidget,
  normalizeSidebarWidgetOrder,
  resolveSidebarExpandedWidget,
} from './sidebar-projection.ts';
import type { SidebarWidgetId } from './types.ts';

test('normalizes sidebar widget order without duplicates or unknown ids', () => {
  assert.deepEqual(normalizeSidebarWidgetOrder(), []);
  assert.deepEqual(
    normalizeSidebarWidgetOrder([
      'settings',
      'settings',
      'unknown' as SidebarWidgetId,
    ]),
    ['settings'],
  );
});

test('inserts a tile-owned widget into a bounded sidebar position', () => {
  assert.deepEqual(
    insertSidebarWidget(['account-menu', 'settings'], 'swap', 1),
    ['account-menu', 'swap', 'settings'],
  );
  assert.deepEqual(insertSidebarWidget(['swap', 'settings'], 'swap', 99), [
    'settings',
    'swap',
  ]);
});

test('moves one sidebar widget with bounded immutable ordering', () => {
  const order: SidebarWidgetId[] = ['account-menu', 'settings'];
  assert.deepEqual(moveSidebarWidget(order, 'account-menu', 99), [
    'settings',
    'account-menu',
  ]);
  assert.deepEqual(moveSidebarWidget(order, 'settings', -1), [
    'settings',
    'account-menu',
  ]);
  assert.deepEqual(order, ['account-menu', 'settings']);
});

test('preserves explicit collapse and repairs stale sidebar expansion', () => {
  const order: SidebarWidgetId[] = ['settings', 'account-menu'];
  assert.equal(resolveSidebarExpandedWidget(order, null), null);
  assert.equal(
    resolveSidebarExpandedWidget(order, 'account-menu'),
    'account-menu',
  );
  assert.equal(
    resolveSidebarExpandedWidget(order, 'unknown' as SidebarWidgetId),
    'settings',
  );
});
