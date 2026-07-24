import { expect, test } from '@playwright/test';

test.use({ viewport: { width: 800, height: 300 } });

test('flipped tooltip mirrors the inset white arrow at its top edge', async ({
  page,
}) => {
  await page.goto('/');

  await page.locator('footer').evaluate((footer) => {
    footer.style.position = 'fixed';
    footer.style.top = '0';
    footer.style.bottom = 'auto';
    footer.style.zIndex = '100';
  });
  await page.getByRole('button', { name: /^Network:/ }).hover();

  const content = page.locator('[data-tooltip-content]');
  await expect(content).toBeVisible();
  await expect(content).toHaveAttribute('data-side', 'bottom');
  const arrows = content.locator('span[data-side]');
  await expect(arrows).toHaveCount(2);
  await expect(arrows.nth(1)).toHaveAttribute('data-side', 'bottom');

  const insetArrowStyle = await arrows.nth(1).evaluate((arrow) => {
    const style = getComputedStyle(arrow);
    return { color: style.color, translate: style.translate };
  });
  expect(insetArrowStyle).toEqual({
    color: 'rgb(255, 255, 255)',
    translate: '0px 3px',
  });
});
