import { type Page, expect, test } from '@playwright/test';

const FRAME_STORAGE_KEY = 'deos-workspace-frame';
const PANEL_LABELS = [
  'Swap',
  'Wallet',
  'Log',
  'Statistics',
  'Chart',
  'Automation',
  'Governance',
  'Wiki',
] as const;

async function seedCollapsedFrame(page: Page, trackWrites = false) {
  await page.addInitScript(
    ({ frameKey, track }) => {
      localStorage.removeItem('deos-tile-layout');
      localStorage.setItem(
        frameKey,
        JSON.stringify({
          sidebar: { open: false },
          mobile: { panelOrder: [], expandedPanelId: null },
        }),
      );
      if (!track) return;
      (
        window as typeof window & { __mobileFrameWrites: number }
      ).__mobileFrameWrites = 0;
      const originalSetItem = Storage.prototype.setItem;
      Storage.prototype.setItem = function (key, value) {
        if (this === localStorage && key === frameKey) {
          (
            window as typeof window & { __mobileFrameWrites: number }
          ).__mobileFrameWrites += 1;
        }
        return originalSetItem.call(this, key, value);
      };
    },
    { frameKey: FRAME_STORAGE_KEY, track: trackWrites },
  );
}

async function mobilePanelOrder(page: Page): Promise<string[]> {
  return page
    .locator('[data-mobile-panel-id]')
    .evaluateAll((panels) =>
      panels.map((panel) => (panel as HTMLElement).dataset.mobilePanelId ?? ''),
    );
}

async function sidebarWidgetOrder(page: Page): Promise<string[]> {
  return page
    .locator('[data-sidebar-widget-id]')
    .evaluateAll((widgets) =>
      widgets.map(
        (widget) => (widget as HTMLElement).dataset.sidebarWidgetId ?? '',
      ),
    );
}

test.use({ viewport: { width: 390, height: 844 } });

test('mobile headings can expand, collapse, and reopen from a deep link', async ({
  page,
}) => {
  await seedCollapsedFrame(page);
  await page.goto('/');

  const disclosureHeaders = page.locator(
    'button[aria-controls^="mobile-panel-content-"]',
  );
  await expect(disclosureHeaders).toHaveCount(8);
  for (const header of await disclosureHeaders.all()) {
    await expect(header).toHaveAttribute('aria-expanded', 'false');
    await expect(header.locator('svg')).toHaveCount(2);
  }

  const swapHeader = page.getByRole('button', { name: 'Swap', exact: true });
  await swapHeader.click();
  await expect(swapHeader).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByRole('region', { name: 'Swap' })).toBeVisible();

  await swapHeader.click();
  await expect(swapHeader).toHaveAttribute('aria-expanded', 'false');
  await expect(page.getByRole('region', { name: 'Swap' })).toHaveCount(0);
  expect(
    await page.evaluate((frameKey) => {
      const frame = JSON.parse(localStorage.getItem(frameKey) ?? 'null') as {
        mobile?: { expandedPanelId?: string | null };
      } | null;
      return frame?.mobile?.expandedPanelId;
    }, FRAME_STORAGE_KEY),
  ).toBeNull();

  await page.goto('/#wiki');
  const wikiHeader = page.getByRole('button', { name: 'Wiki', exact: true });
  await expect(wikiHeader).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByRole('region', { name: 'Wiki' })).toBeVisible();
});

test('compact tile mode overlays the sidebar shelf without reflowing tiles', async ({
  page,
}) => {
  await seedCollapsedFrame(page);
  await page.setViewportSize({ width: 800, height: 700 });
  await page.goto('/');

  await expect(page.getByLabel('Mobile workspace widgets')).toHaveCount(0);
  const accountButton = page.getByRole('button', { name: 'Open sidebar' });
  await expect(accountButton).toHaveText('Alice');
  await expect(accountButton.locator('svg')).toHaveCount(2);
  await expect(accountButton.locator('svg').first()).toHaveClass(
    /lucide-user-round/,
  );
  await expect(accountButton.locator('svg').last()).toHaveClass(
    /lucide-chevron-up/,
  );
  const accountPadding = await accountButton.evaluate((button) => {
    const style = getComputedStyle(button);
    return [
      style.paddingTop,
      style.paddingRight,
      style.paddingBottom,
      style.paddingLeft,
    ];
  });
  expect(new Set(accountPadding).size).toBe(1);
  await expect(page.getByRole('button', { name: 'Reset layout' })).toHaveCount(
    0,
  );
  const workspace = page.locator('main');
  const before = await workspace.boundingBox();
  expect(before).not.toBeNull();

  await accountButton.click();
  await expect(
    page.getByRole('button', { name: 'Close sidebar' }).locator('svg').last(),
  ).toHaveClass(/lucide-chevron-down/);
  const sidebar = page.getByRole('dialog', {
    name: 'Account and settings',
  });
  await expect(sidebar).toBeVisible();
  await expect(sidebar.getByText('Sidebar', { exact: true })).toHaveCount(0);
  await expect(
    sidebar.getByRole('button', { name: 'Close sidebar' }),
  ).toHaveCount(0);
  const accountSection = sidebar.getByRole('button', {
    name: 'Account',
    exact: true,
  });
  const settingsSection = sidebar.getByRole('button', {
    name: 'Settings',
    exact: true,
  });
  await expect(accountSection).toHaveAttribute('aria-expanded', 'true');
  await expect(settingsSection).toHaveAttribute('aria-expanded', 'false');
  await expect(accountSection.locator('svg')).toHaveCount(2);
  await expect(settingsSection.locator('svg')).toHaveCount(2);
  const accountReorderGrip = sidebar.getByRole('button', {
    name: /^Reorder Account,/,
  });
  const settingsReorderGrip = sidebar.getByRole('button', {
    name: /^Reorder Settings,/,
  });
  await expect(accountReorderGrip.locator('svg')).toHaveClass(
    /lucide-grip-vertical/,
  );
  await expect(settingsReorderGrip.locator('svg')).toHaveClass(
    /lucide-grip-vertical/,
  );
  await expect(accountReorderGrip).toHaveCSS('width', '44px');
  await expect(settingsReorderGrip).toHaveCSS('width', '44px');
  await expect(sidebar.getByRole('region', { name: 'Account' })).toBeVisible();
  await expect(
    sidebar.getByRole('button', { name: 'Reset layout' }),
  ).toHaveCount(0);

  await settingsSection.click();
  await expect(accountSection).toHaveAttribute('aria-expanded', 'false');
  await expect(settingsSection).toHaveAttribute('aria-expanded', 'true');
  await expect(sidebar.getByRole('region', { name: 'Settings' })).toBeVisible();
  await expect(
    sidebar.getByRole('button', { name: 'Reset layout' }),
  ).toBeVisible();
  await settingsSection.click();
  await expect(settingsSection).toHaveAttribute('aria-expanded', 'false');
  await expect(sidebar.getByRole('region')).toHaveCount(0);

  await page.keyboard.press('Escape');
  await expect(sidebar).toHaveCount(0);
  await page.getByRole('button', { name: 'Open sidebar' }).click();
  await expect(accountSection).toHaveAttribute('aria-expanded', 'true');
  await expect(sidebar.getByRole('region', { name: 'Account' })).toBeVisible();

  await expect(page.locator('[data-dialog-overlay]')).toHaveCSS(
    'backdrop-filter',
    'none',
  );
  await expect(sidebar.locator('div[aria-hidden="true"]')).toHaveCount(0);
  const after = await workspace.boundingBox();
  expect(after).toEqual(before);
  await expect(sidebar).toHaveCSS('position', 'fixed');
  const sidebarBox = await sidebar.boundingBox();
  expect(sidebarBox).not.toBeNull();
  expect(sidebarBox?.width).toBe(800);
  expect(sidebarBox?.y).toBeGreaterThan(0);
});

test('persisted placement partitions widgets between tiles and sidebar without duplication', async ({
  page,
}) => {
  await page.addInitScript((frameKey) => {
    localStorage.removeItem('deos-tile-layout');
    localStorage.setItem(
      frameKey,
      JSON.stringify({
        sidebar: {
          placementVersion: 1,
          open: true,
          widgetOrder: ['swap', 'account-menu', 'settings'],
          expandedWidgetId: 'swap',
        },
        mobile: { panelOrder: [], expandedPanelId: null },
      }),
    );
  }, FRAME_STORAGE_KEY);
  await page.setViewportSize({ width: 1200, height: 800 });
  await page.goto('/');

  const sidebar = page.getByLabel('Sidebar widgets');
  await expect(
    sidebar.locator('[data-sidebar-widget-id="swap"]'),
  ).toBeVisible();
  await expect(sidebar.getByRole('region', { name: 'Swap' })).toBeVisible();
  await expect(page.locator('[data-tab-id="swap"]')).toHaveCount(0);
  const placedWidgetIds = await page
    .locator('[data-tab-id], [data-sidebar-widget-id]')
    .evaluateAll((elements) =>
      elements.map(
        (element) =>
          (element as HTMLElement).dataset.tabId ??
          (element as HTMLElement).dataset.sidebarWidgetId ??
          '',
      ),
    );
  expect(placedWidgetIds).toHaveLength(10);
  expect(new Set(placedWidgetIds).size).toBe(10);

  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.locator('[data-mobile-panel-id="swap"]')).toHaveCount(0);
  await expect(page.locator('[data-mobile-panel-id]')).toHaveCount(7);
});

test('desktop moves widgets between tile tabs and the sidebar without loss', async ({
  page,
}) => {
  await seedCollapsedFrame(page, true);
  await page.setViewportSize({ width: 1200, height: 800 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Open sidebar' }).click();
  await page.evaluate(() => {
    (
      window as typeof window & { __mobileFrameWrites: number }
    ).__mobileFrameWrites = 0;
  });

  const sidebar = page.getByLabel('Sidebar widgets');
  await page
    .getByRole('tab', { name: 'Swap', exact: true })
    .dragTo(sidebar.locator('[data-sidebar-widget-id="settings"]'));
  await expect(page.locator('[data-tab-id="swap"]')).toHaveCount(0);
  await expect(
    sidebar.locator('[data-sidebar-widget-id="swap"]'),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __mobileFrameWrites: number })
          .__mobileFrameWrites,
    ),
  ).toBe(1);

  await sidebar
    .getByRole('button', { name: /^Reorder Account,/ })
    .dragTo(page.getByRole('tablist').first());
  await expect(
    sidebar.locator('[data-sidebar-widget-id="account-menu"]'),
  ).toHaveCount(0);
  await expect(page.locator('[data-tab-id="account-menu"]')).toBeVisible();

  const settingsGrip = sidebar.getByRole('button', {
    name: /^Reorder Settings,/,
  });
  await settingsGrip.focus();
  await settingsGrip.press('Shift+ArrowLeft');
  const settingsTab = page.locator('[data-tab-id="settings"]');
  await expect(settingsTab).toBeVisible();
  await expect(settingsTab).toBeFocused();
  await settingsTab.press('Shift+ArrowRight');
  const restoredSettingsGrip = sidebar.getByRole('button', {
    name: /^Reorder Settings,/,
  });
  await expect(restoredSettingsGrip).toBeVisible();
  await expect(restoredSettingsGrip).toBeFocused();

  const placedWidgetIds = await page
    .locator('[data-tab-id], [data-sidebar-widget-id]')
    .evaluateAll((elements) =>
      elements.map(
        (element) =>
          (element as HTMLElement).dataset.tabId ??
          (element as HTMLElement).dataset.sidebarWidgetId ??
          '',
      ),
    );
  expect(placedWidgetIds).toHaveLength(10);
  expect(new Set(placedWidgetIds).size).toBe(10);
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __mobileFrameWrites: number })
          .__mobileFrameWrites,
    ),
  ).toBe(4);
});

test('wide sidebar trigger points left to open and right to close', async ({
  page,
}) => {
  await seedCollapsedFrame(page);
  await page.setViewportSize({ width: 1200, height: 800 });
  await page.goto('/');

  const openButton = page.getByRole('button', { name: 'Open sidebar' });
  await expect(openButton.locator('svg').last()).toHaveClass(
    /lucide-chevron-left/,
  );
  await openButton.click();
  const closeButton = page.getByRole('button', { name: 'Close sidebar' });
  await expect(closeButton.locator('svg').last()).toHaveClass(
    /lucide-chevron-right/,
  );
  const sidebar = page.getByLabel('Sidebar widgets');
  await expect(sidebar).toBeVisible();
  const accountDisclosure = sidebar.getByRole('button', {
    name: 'Account',
    exact: true,
  });
  await expect(accountDisclosure).toHaveAttribute('aria-expanded', 'true');
  await accountDisclosure.click();
  await expect(accountDisclosure).toHaveAttribute('aria-expanded', 'false');
  await closeButton.click();
  await page.getByRole('button', { name: 'Open sidebar' }).click();
  await expect(accountDisclosure).toHaveAttribute('aria-expanded', 'true');
});

test('sidebar labels reorder with preview, one commit, cancel restoration, and keyboard focus', async ({
  page,
}) => {
  await seedCollapsedFrame(page, true);
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.setViewportSize({ width: 800, height: 700 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Open sidebar' }).click();
  const tileStateBefore = await page.evaluate(() =>
    localStorage.getItem('deos-tile-layout'),
  );
  await page.evaluate(() => {
    (
      window as typeof window & { __mobileFrameWrites: number }
    ).__mobileFrameWrites = 0;
  });

  const accountLabel = page.getByRole('button', {
    name: /^Reorder Account,/,
  });
  const settingsSection = page.locator('[data-sidebar-widget-id="settings"]');
  await accountLabel.hover();
  await page.mouse.down();
  await settingsSection.hover();
  await expect(page.locator('[data-drop-preview="true"]')).toHaveAttribute(
    'data-sidebar-widget-id',
    'account-menu',
  );
  await expect
    .poll(() => sidebarWidgetOrder(page))
    .toEqual(['settings', 'account-menu']);
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __mobileFrameWrites: number })
          .__mobileFrameWrites,
    ),
  ).toBe(0);

  await page.mouse.up();
  await expect
    .poll(() => sidebarWidgetOrder(page))
    .toEqual(['settings', 'account-menu']);
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __mobileFrameWrites: number })
          .__mobileFrameWrites,
    ),
  ).toBe(1);
  await expect(accountLabel).toBeFocused();
  await expect
    .poll(() =>
      page
        .locator('[data-sidebar-widget-id]')
        .evaluateAll((widgets) =>
          widgets.reduce(
            (count, widget) =>
              count +
              widget
                .getAnimations()
                .filter((animation) => animation.playState === 'running')
                .length,
            0,
          ),
        ),
    )
    .toBe(0);
  expect(
    await page.evaluate(() => localStorage.getItem('deos-tile-layout')),
  ).toBe(tileStateBefore);

  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Open sidebar' }).click();
  await expect(
    page.getByRole('button', { name: 'Settings', exact: true }),
  ).toHaveAttribute('aria-expanded', 'true');
  await expect(
    page.getByRole('button', { name: 'Account', exact: true }),
  ).toHaveAttribute('aria-expanded', 'false');

  await page.evaluate(() => {
    (
      window as typeof window & { __mobileFrameWrites: number }
    ).__mobileFrameWrites = 0;
  });
  const movedSettingsBox = await settingsSection.boundingBox();
  expect(movedSettingsBox).not.toBeNull();
  if (!movedSettingsBox) return;
  await accountLabel.hover();
  await page.mouse.down();
  await settingsSection.hover();
  await expect
    .poll(() => sidebarWidgetOrder(page))
    .toEqual(['account-menu', 'settings']);
  await accountLabel.dispatchEvent('pointercancel', {
    pointerId: 1,
    pointerType: 'mouse',
    clientX: movedSettingsBox.x + movedSettingsBox.width / 2,
    clientY: movedSettingsBox.y + movedSettingsBox.height / 2,
  });
  await page.mouse.up();
  await expect
    .poll(() => sidebarWidgetOrder(page))
    .toEqual(['settings', 'account-menu']);
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __mobileFrameWrites: number })
          .__mobileFrameWrites,
    ),
  ).toBe(0);

  await expect(
    page.getByRole('dialog', { name: 'Account and settings' }),
  ).toBeVisible();
  await accountLabel.press('ArrowUp');
  await expect
    .poll(() => sidebarWidgetOrder(page))
    .toEqual(['account-menu', 'settings']);
  await expect(accountLabel).toBeFocused();
  await expect(
    page.getByText('Account moved to position 1 of 2'),
  ).toBeAttached();
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __mobileFrameWrites: number })
          .__mobileFrameWrites,
    ),
  ).toBe(1);
});

test('pointer reorder previews with displacement, commits once, and cancels cleanly', async ({
  page,
}) => {
  await seedCollapsedFrame(page, true);
  await page.goto('/');

  const swapGrip = page.getByRole('button', { name: /^Reorder Swap,/ });
  const logPanel = page.locator('[data-mobile-panel-id="log"]');
  const logBox = await logPanel.boundingBox();
  const gripBox = await swapGrip.boundingBox();
  expect(logBox).not.toBeNull();
  expect(gripBox).not.toBeNull();
  if (!logBox || !gripBox) return;

  await page.mouse.move(
    gripBox.x + gripBox.width / 2,
    gripBox.y + gripBox.height / 2,
  );
  await page.mouse.down();
  await page.mouse.move(
    logBox.x + logBox.width / 2,
    logBox.y + logBox.height / 2,
  );
  await expect(page.locator('[data-drop-preview="true"]')).toHaveAttribute(
    'data-mobile-panel-id',
    'swap',
  );
  await expect
    .poll(() => mobilePanelOrder(page))
    .toEqual([
      'wallet',
      'log',
      'swap',
      'statistics',
      'chart',
      'automation',
      'governance',
      'wiki',
    ]);
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __mobileFrameWrites: number })
          .__mobileFrameWrites,
    ),
  ).toBe(0);

  await page.mouse.up();
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __mobileFrameWrites: number })
          .__mobileFrameWrites,
    ),
  ).toBe(1);
  const committedOrder = await mobilePanelOrder(page);
  expect(committedOrder.slice(0, 3)).toEqual(['wallet', 'log', 'swap']);
  await expect
    .poll(() =>
      page
        .locator('[data-mobile-panel-id]')
        .evaluateAll((panels) =>
          panels.reduce(
            (count, panel) =>
              count +
              panel
                .getAnimations()
                .filter((animation) => animation.playState === 'running')
                .length,
            0,
          ),
        ),
    )
    .toBe(0);

  await page.evaluate(() => {
    (
      window as typeof window & { __mobileFrameWrites: number }
    ).__mobileFrameWrites = 0;
  });
  const statisticsPanel = page.locator('[data-mobile-panel-id="statistics"]');
  const statisticsBox = await statisticsPanel.boundingBox();
  const movedGripBox = await swapGrip.boundingBox();
  expect(statisticsBox).not.toBeNull();
  expect(movedGripBox).not.toBeNull();
  if (!statisticsBox || !movedGripBox) return;
  await page.mouse.move(
    movedGripBox.x + movedGripBox.width / 2,
    movedGripBox.y + movedGripBox.height / 2,
  );
  await page.mouse.down();
  await page.mouse.move(
    statisticsBox.x + statisticsBox.width / 2,
    statisticsBox.y + statisticsBox.height / 2,
  );
  await expect.poll(() => mobilePanelOrder(page)).not.toEqual(committedOrder);
  await swapGrip.dispatchEvent('pointercancel', {
    pointerId: 1,
    pointerType: 'mouse',
    clientX: statisticsBox.x + statisticsBox.width / 2,
    clientY: statisticsBox.y + statisticsBox.height / 2,
  });
  await page.mouse.up();
  await expect.poll(() => mobilePanelOrder(page)).toEqual(committedOrder);
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __mobileFrameWrites: number })
          .__mobileFrameWrites,
    ),
  ).toBe(0);
});

test('expanded tasks use intrinsic height and cap long readers', async ({
  page,
}) => {
  await seedCollapsedFrame(page);
  await page.goto('/');

  await page.getByRole('button', { name: 'Swap', exact: true }).click();
  const swapContent = page.locator('#mobile-panel-content-swap');
  await expect(swapContent).toBeVisible();
  const swapSize = await swapContent.evaluate((content) => {
    const scrollHost = content.firstElementChild?.firstElementChild;
    if (!(scrollHost instanceof HTMLElement)) return null;
    return {
      contentHeight: content.clientHeight,
      clientHeight: scrollHost.clientHeight,
      scrollHeight: scrollHost.scrollHeight,
    };
  });
  expect(swapSize).not.toBeNull();
  if (!swapSize) return;
  expect(swapSize.contentHeight).toBeLessThan(160);
  expect(swapSize.scrollHeight).toBeLessThanOrEqual(swapSize.clientHeight + 1);

  await page.getByRole('button', { name: 'Wiki', exact: true }).click();
  const wikiContent = page.locator('#mobile-panel-content-wiki');
  await expect(wikiContent).toBeVisible();
  await expect
    .poll(() =>
      wikiContent.evaluate((content) => {
        const scrollHost = content.firstElementChild?.firstElementChild;
        return scrollHost instanceof HTMLElement
          ? scrollHost.scrollHeight - scrollHost.clientHeight
          : 0;
      }),
    )
    .toBeGreaterThan(0);
  const wikiHostHeight = await wikiContent.evaluate((content) => {
    const scrollHost = content.firstElementChild?.firstElementChild;
    return scrollHost instanceof HTMLElement ? scrollHost.clientHeight : 0;
  });
  expect(wikiHostHeight).toBeLessThanOrEqual(844 * 0.68 + 1);
});

test('mobile workspace scrolls behind chrome with clear resting endpoints', async ({
  page,
}) => {
  await seedCollapsedFrame(page);
  await page.goto('/');
  await page.getByRole('button', { name: 'Wiki', exact: true }).click();

  const workspace = page.getByLabel('Mobile workspace widgets');
  const header = page.locator('header');
  const footer = page.locator('footer');
  const firstPanel = page.locator('[data-mobile-panel-id]').first();
  const lastPanel = page.locator('[data-mobile-panel-id]').last();
  await expect(workspace).toBeVisible();
  const workspaceBox = await workspace.boundingBox();
  const headerBox = await header.boundingBox();
  const footerBox = await footer.boundingBox();
  const firstBox = await firstPanel.boundingBox();
  expect(workspaceBox).not.toBeNull();
  expect(headerBox).not.toBeNull();
  expect(footerBox).not.toBeNull();
  expect(firstBox).not.toBeNull();
  if (!workspaceBox || !headerBox || !footerBox || !firstBox) return;

  const snapContract = await workspace.evaluate((element) => {
    const style = getComputedStyle(element);
    const firstSection = element.querySelector<HTMLElement>(
      '[data-mobile-panel-id]',
    );
    return {
      type: style.scrollSnapType,
      paddingTop: Number.parseFloat(style.scrollPaddingTop),
      firstAlign: firstSection
        ? getComputedStyle(firstSection).scrollSnapAlign
        : '',
    };
  });
  expect(snapContract.type).toBe('y');
  expect(snapContract.type).not.toContain('mandatory');
  expect(snapContract.firstAlign).toBe('start');
  expect(snapContract.paddingTop).toBeGreaterThanOrEqual(headerBox.height + 10);

  expect(workspaceBox.x).toBe(0);
  expect(workspaceBox.y).toBe(0);
  expect(workspaceBox.width).toBe(390);
  expect(workspaceBox.height).toBe(844);
  const firstPanelLeftGap = firstBox.x;
  const firstPanelRightGap = 390 - (firstBox.x + firstBox.width);
  expect(firstPanelLeftGap).toBeGreaterThanOrEqual(12);
  expect(Math.abs(firstPanelLeftGap - firstPanelRightGap)).toBeLessThanOrEqual(
    1,
  );
  expect(firstBox.y).toBeGreaterThanOrEqual(
    headerBox.y + headerBox.height + 10,
  );

  const scrollState = await workspace.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
    return {
      clientHeight: element.clientHeight,
      scrollHeight: element.scrollHeight,
      scrollTop: element.scrollTop,
    };
  });
  expect(scrollState.scrollHeight).toBeGreaterThan(scrollState.clientHeight);
  expect(scrollState.scrollTop).toBeGreaterThan(0);
  const lastBox = await lastPanel.boundingBox();
  expect(lastBox).not.toBeNull();
  if (!lastBox) return;
  expect(lastBox.y + lastBox.height).toBeLessThanOrEqual(footerBox.y - 10);
});

test('global readiness stays in Status instead of repeating in task bodies', async ({
  page,
}) => {
  await seedCollapsedFrame(page);
  await page.goto('/');

  await expect(page.getByRole('button', { name: /^Network:/ })).toBeVisible();
  const repeatedReadiness =
    /Preparing chain data|Waiting for chain data|Connect a DEOS network|Chain data unavailable/;

  for (const label of PANEL_LABELS) {
    await page.getByRole('button', { name: label, exact: true }).click();
    const region = page.getByRole('region', { name: label, exact: true });
    await expect(region).toBeVisible();
    expect(await region.innerText()).not.toMatch(repeatedReadiness);
  }
});
