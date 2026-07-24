import { type Page, expect, test } from '@playwright/test';

type Direction = 'horizontal' | 'vertical';
type PanelId =
  | 'swap'
  | 'chart'
  | 'statistics'
  | 'log'
  | 'governance'
  | 'wallet'
  | 'automation'
  | 'wiki';

type TileNode =
  | {
      type: 'leaf';
      id: string;
      tabs: PanelId[];
      activeTab: PanelId;
    }
  | {
      type: 'split';
      id: string;
      direction: Direction;
      ratio: number;
      children: [TileNode, TileNode];
    };

const TILE_LAYOUT_STORAGE_KEY = 'deos-tile-layout';

function threePaneLayout(direction: Direction): TileNode {
  const leaf = (activeTab: PanelId, tabs: PanelId[]): TileNode => ({
    type: 'leaf',
    id: `browser-${activeTab}`,
    tabs,
    activeTab,
  });
  return {
    type: 'split',
    id: 'browser-root',
    direction,
    ratio: 1 / 3,
    children: [
      leaf('swap', ['swap', 'wallet', 'governance']),
      {
        type: 'split',
        id: 'browser-tail',
        direction,
        ratio: 1 / 2,
        children: [
          leaf('chart', ['chart', 'automation', 'wiki']),
          leaf('log', ['log', 'statistics']),
        ],
      },
    ],
  };
}

async function openLayout(page: Page, direction: Direction) {
  await page.addInitScript(
    ({ key, layout }) => {
      localStorage.setItem(key, JSON.stringify(layout));
      (
        window as typeof window & { __tileLayoutWrites: number }
      ).__tileLayoutWrites = 0;
      const originalSetItem = Storage.prototype.setItem;
      Storage.prototype.setItem = function (storageKey, value) {
        if (this === localStorage && storageKey === key) {
          (
            window as typeof window & { __tileLayoutWrites: number }
          ).__tileLayoutWrites += 1;
        }
        return originalSetItem.call(this, storageKey, value);
      };
    },
    { key: TILE_LAYOUT_STORAGE_KEY, layout: threePaneLayout(direction) },
  );
  await page.goto('/');
  await expect(page.locator('[data-split-segment]')).toHaveCount(3);
}

async function segmentSnapshot(page: Page) {
  return page.locator('[data-split-segment]').evaluateAll((segments) =>
    segments.map((segment) => {
      const element = segment as HTMLElement;
      const rect = element.getBoundingClientRect();
      return {
        id: element.dataset.splitSegment,
        width: rect.width,
        height: rect.height,
        basis: element.style.flexBasis,
        grow: element.style.flexGrow,
        shrink: element.style.flexShrink,
        weight: element.dataset.splitWeight,
      };
    }),
  );
}

function expectNear(actual: number, expected: number, tolerance = 2) {
  expect(Math.abs(actual - expected)).toBeLessThanOrEqual(tolerance);
}

test('fine-pointer resize hit area stays inside the visible 12px lane', async ({
  page,
}) => {
  await openLayout(page, 'horizontal');
  const swapTab = page.getByRole('tab', { name: 'Swap' });
  await expect(swapTab.locator('svg')).toHaveCount(1);
  expect(
    await swapTab.evaluate((tab) => getComputedStyle(tab).flexDirection),
  ).toBe('row');
  const handle = page.getByRole('separator').nth(1);
  const box = await handle.boundingBox();
  expect(box).not.toBeNull();
  if (!box) return;
  expect(box.width).toBe(12);

  const hitTargets = await page.evaluate(
    ({ centerX, centerY }) =>
      [-10, 0, 10].map((offset) =>
        document
          .elementFromPoint(centerX + offset, centerY)
          ?.closest('[role="separator"]')
          ? 'separator'
          : 'pane',
      ),
    {
      centerX: box.x + box.width / 2,
      centerY: box.y + box.height / 2,
    },
  );
  expect(hitTargets).toEqual(['pane', 'separator', 'pane']);
});

test('middle handle freezes the group, previews adjacent panes only, and commits once', async ({
  page,
}) => {
  await openLayout(page, 'horizontal');
  const handle = page.getByRole('separator').nth(1);
  const box = await handle.boundingBox();
  expect(box).not.toBeNull();
  if (!box) return;

  const before = await segmentSnapshot(page);
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();

  const frozen = await segmentSnapshot(page);
  for (let index = 0; index < frozen.length; index += 1) {
    expect(frozen[index].grow).toBe('0');
    expect(frozen[index].shrink).toBe('0');
    expectNear(Number.parseFloat(frozen[index].basis), before[index].width);
  }
  await expect
    .poll(() => handle.evaluate((element) => element.hasPointerCapture(1)))
    .toBe(true);

  await page.mouse.move(box.x + box.width / 2 + 80, box.y + box.height / 2);
  await page.evaluate(() => new Promise(requestAnimationFrame));
  const preview = await segmentSnapshot(page);
  expectNear(preview[0].width, before[0].width);
  expectNear(preview[1].width, before[1].width + 80);
  expectNear(preview[2].width, before[2].width - 80);

  await page.mouse.up();
  const restored = await segmentSnapshot(page);
  for (const segment of restored) {
    expect(segment.basis).toBe('0px');
    expectNear(
      Number.parseFloat(segment.grow),
      Number(segment.weight),
      0.000001,
    );
    expect(segment.shrink).toBe('1');
  }
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __tileLayoutWrites: number })
          .__tileLayoutWrites,
    ),
  ).toBe(1);
  await expect
    .poll(() => handle.evaluate((element) => element.hasPointerCapture(1)))
    .toBe(false);
});

test('pointer cancel restores the frozen DOM without persisting preview sizes', async ({
  page,
}) => {
  await openLayout(page, 'horizontal');
  const handle = page.getByRole('separator').first();
  const box = await handle.boundingBox();
  expect(box).not.toBeNull();
  if (!box) return;

  const before = await segmentSnapshot(page);
  const storedBefore = await page.evaluate(
    (key) => localStorage.getItem(key),
    TILE_LAYOUT_STORAGE_KEY,
  );
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 60, box.y + box.height / 2);
  await page.evaluate(() => new Promise(requestAnimationFrame));
  await handle.dispatchEvent('pointercancel', {
    pointerId: 1,
    pointerType: 'mouse',
    clientX: box.x + box.width / 2 + 60,
    clientY: box.y + box.height / 2,
  });
  await page.mouse.up();

  const restored = await segmentSnapshot(page);
  for (let index = 0; index < restored.length; index += 1) {
    expect(restored[index].basis).toBe('0px');
    expectNear(
      Number.parseFloat(restored[index].grow),
      Number(restored[index].weight),
      0.000001,
    );
    expect(restored[index].shrink).toBe('1');
    expectNear(restored[index].width, before[index].width);
  }
  expect(
    await page.evaluate(
      (key) => localStorage.getItem(key),
      TILE_LAYOUT_STORAGE_KEY,
    ),
  ).toBe(storedBefore);
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __tileLayoutWrites: number })
          .__tileLayoutWrites,
    ),
  ).toBe(0);
});

test('touch drag captures the pointer and commits one final layout', async ({
  context,
  page,
}) => {
  await openLayout(page, 'horizontal');
  await page.evaluate(() => {
    (
      window as typeof window & { __lastPointer?: { id: number; type: string } }
    ).__lastPointer = undefined;
    document.addEventListener(
      'pointerdown',
      (event) => {
        (
          window as typeof window & {
            __lastPointer?: { id: number; type: string };
          }
        ).__lastPointer = { id: event.pointerId, type: event.pointerType };
      },
      { capture: true, once: true },
    );
  });
  const handle = page.getByRole('separator').first();
  const box = await handle.boundingBox();
  expect(box).not.toBeNull();
  if (!box) return;
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  const client = await context.newCDPSession(page);
  await client.send('Emulation.setTouchEmulationEnabled', {
    enabled: true,
    maxTouchPoints: 1,
  });
  await client.send('Input.dispatchTouchEvent', {
    type: 'touchStart',
    touchPoints: [{ x, y, id: 7 }],
  });

  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (
            window as typeof window & {
              __lastPointer?: { id: number; type: string };
            }
          ).__lastPointer ?? null,
      ),
    )
    .not.toBeNull();
  const pointerState = await page.evaluate(
    () =>
      (
        window as typeof window & {
          __lastPointer?: { id: number; type: string };
        }
      ).__lastPointer ?? null,
  );
  expect(pointerState?.type).toBe('touch');
  expect(pointerState).not.toBeNull();
  if (!pointerState) return;
  await expect(
    handle.evaluate(
      (element, pointerId) => element.hasPointerCapture(pointerId),
      pointerState.id,
    ),
  ).resolves.toBe(true);

  await client.send('Input.dispatchTouchEvent', {
    type: 'touchMove',
    touchPoints: [{ x: x + 50, y, id: 7 }],
  });
  await client.send('Input.dispatchTouchEvent', {
    type: 'touchEnd',
    touchPoints: [],
  });
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __tileLayoutWrites: number })
          .__tileLayoutWrites,
    ),
  ).toBe(1);
});

test('constrained viewport scrolls the complete recursive minimum-height tree', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 300 });
  await openLayout(page, 'vertical');
  const result = await page
    .locator('[data-split-segment]')
    .first()
    .evaluate((segment) => {
      const group = segment.parentElement;
      const root = group?.parentElement?.parentElement;
      if (!(root instanceof HTMLElement)) return null;
      const segmentHeights = Array.from(
        group?.querySelectorAll(':scope > [data-split-segment]') ?? [],
      ).map(
        (element) => (element as HTMLElement).getBoundingClientRect().height,
      );
      root.scrollTop = root.scrollHeight;
      return {
        clientHeight: root.clientHeight,
        scrollHeight: root.scrollHeight,
        scrollTop: root.scrollTop,
        segmentHeights,
      };
    });
  expect(result).not.toBeNull();
  if (!result) return;
  expect(result.clientHeight).toBeLessThan(result.scrollHeight);
  expect(result.scrollHeight).toBeGreaterThanOrEqual(312);
  expect(result.scrollTop).toBeGreaterThan(0);
  for (const height of result.segmentHeights) {
    expectNear(height, 96);
  }
});
