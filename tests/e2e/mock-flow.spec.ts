import { test, expect, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

async function preview(page: Page) {
  await page.getByRole('button', { name: 'Try a sample link' }).click();
  await page.getByRole('button', { name: 'Find video' }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
}
async function enqueue(page: Page) {
  await preview(page);
  await page.getByRole('button', { name: 'Add mock download' }).click();
  await expect(
    page.getByRole('heading', { name: 'Your downloads.' }),
  ).toBeVisible();
}
test.beforeEach(async ({ page }) => {
  await page.clock.install();
  await page.goto('/');
  await expect(page.getByRole('button', { name: 'Find video' })).toBeEnabled();
});

test('validates links, previews formats, and supports keyboard dismissal', async ({
  page,
}) => {
  await page.getByRole('link', { name: 'Skip to content' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('main')).toBeFocused();
  await expect(page.getByRole('button', { name: 'Find video' })).toBeVisible();
  await page
    .getByRole('textbox', { name: 'Video URL' })
    .fill('http://127.0.0.1/private');
  await page.getByRole('button', { name: 'Find video' }).click();
  await expect(page.getByRole('alert')).toContainText('public http or https');
  await preview(page);
  await expect(page.getByRole('radio')).toHaveCount(3);
  await expect(page.getByRole('dialog')).toContainText(
    'no media file will be saved',
  );
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).not.toBeVisible();
  await expect(page.getByRole('textbox', { name: 'Video URL' })).toBeFocused();
});

test('finishes a mock download, keeps history after reload, and filters library', async ({
  page,
}) => {
  await preview(page);
  await page.getByRole('radio', { name: 'HD 720p', exact: false }).check();
  await page.getByRole('button', { name: 'Add mock download' }).click();
  await expect(page.getByRole('article')).toContainText('720p');
  await page.clock.runFor(8_000);
  await expect(page.getByRole('article')).toContainText('Simulated');
  await page.getByRole('link', { name: 'Library', exact: true }).click();
  await expect(page.getByRole('article')).toHaveCount(1);
  await page.reload();
  await expect(page.getByRole('article')).toHaveCount(1);
  await page
    .getByRole('textbox', { name: 'Search your library' })
    .fill('no match');
  await expect(page.getByText('Nothing matches this search.')).toBeVisible();
  await page.getByRole('textbox', { name: 'Search your library' }).fill('');
  await page
    .getByRole('combobox', { name: 'Media type' })
    .selectOption('audio');
  await expect(page.getByRole('article')).toHaveCount(0);
  await page
    .getByRole('combobox', { name: 'Media type' })
    .selectOption('video');
  await expect(page.getByRole('article')).toHaveCount(1);
});

test('cancels without late completion and retries with a new attempt', async ({
  page,
}) => {
  await enqueue(page);
  await page.clock.runFor(700);
  await page.getByRole('button', { name: 'Cancel A little further' }).click();
  await page.clock.runFor(8_000);
  await expect(page.getByRole('article')).toContainText('Cancelled');
  await page.getByRole('button', { name: 'Retry A little further' }).click();
  await page.clock.runFor(8_000);
  await expect(page.getByRole('article')).toContainText('Simulated');
  expect(
    await page.evaluate(
      () =>
        JSON.parse(localStorage.getItem('downpour.browser.v1')!).jobs[0]
          .attempt,
    ),
  ).toBe(2);
});

test('persists settings and respects a single worker across a reload', async ({
  page,
}) => {
  await page.getByRole('link', { name: 'Settings', exact: true }).click();
  await page
    .getByRole('combobox', { name: 'Simultaneous downloads' })
    .selectOption('1');
  await page.getByRole('radio', { name: 'Light', exact: true }).check();
  await page.getByRole('button', { name: 'Save preferences' }).click();
  await expect(
    page.getByText('Preferences saved on this device.'),
  ).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.getByRole('link', { name: 'Overview', exact: true }).click();
  await enqueue(page);
  await page
    .getByRole('link', { name: 'New download', exact: true })
    .first()
    .click();
  await enqueue(page);
  await page.clock.runFor(700);
  await expect(page.getByRole('article')).toHaveCount(2);
  const statuses = await page.evaluate(() =>
    JSON.parse(localStorage.getItem('downpour.browser.v1')!).jobs.map(
      (job: { status: string }) => job.status,
    ),
  );
  expect(statuses).toEqual(['downloading', 'queued']);
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.clock.runFor(16_000);
  await expect(
    page.getByRole('article').filter({ hasText: 'Simulated' }),
  ).toHaveCount(2);
  await page.getByRole('link', { name: 'Settings', exact: true }).click();
  await expect(
    page.getByRole('combobox', { name: 'Simultaneous downloads' }),
  ).toHaveValue('1');
});

test('preserves corrupt preview storage and reports a startup error', async ({
  page,
}) => {
  await page.evaluate(() =>
    localStorage.setItem('downpour.browser.v1', '{broken'),
  );
  await page.reload();
  await expect(page.getByRole('alert')).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Retry startup' }),
  ).toBeVisible();
  await expect(page.getByRole('button', { name: 'Find video' })).toBeDisabled();
  expect(
    await page.evaluate(() => localStorage.getItem('downpour.browser.v1')),
  ).toBe('{broken');
});

test('fits a small window and passes the home/preview accessibility scan', async ({
  page,
}) => {
  await page.setViewportSize({ width: 540, height: 720 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  const home = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21aa'])
    .analyze();
  expect(home.violations).toEqual([]);
  await preview(page);
  const dialog = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21aa'])
    .analyze();
  expect(dialog.violations).toEqual([]);
  await page.keyboard.press('Tab');
  expect(
    await page.evaluate(
      () => !!document.activeElement?.closest('[role="dialog"]'),
    ),
  ).toBe(true);
});

test('loads only local resources during the mock flow', async ({ page }) => {
  const remote: string[] = [];
  page.on('request', (request) => {
    if (
      !request.url().startsWith('http://127.0.0.1:1420/') &&
      !request.url().startsWith('data:')
    )
      remote.push(request.url());
  });
  await page.reload();
  await enqueue(page);
  await page.clock.runFor(8_000);
  expect(remote).toEqual([]);
});

for (const theme of ['dark', 'light'] as const) {
  test(`checks populated screens for accessibility in ${theme} mode`, async ({
    page,
  }) => {
    await enqueue(page);
    await page.clock.runFor(8_000);
    await page.getByRole('link', { name: 'Settings', exact: true }).click();
    if (theme === 'light') {
      await page.getByRole('radio', { name: 'Light', exact: true }).check();
      await page.getByRole('button', { name: 'Save preferences' }).click();
      await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
    }
    for (const screen of ['Settings', 'Overview', 'Downloads', 'Library']) {
      await page.getByRole('link', { name: screen, exact: true }).click();
      const results = await new AxeBuilder({ page })
        .withTags(['wcag2a', 'wcag2aa', 'wcag21aa'])
        .analyze();
      expect(
        results.violations.map((violation) => ({
          id: violation.id,
          nodes: violation.nodes.map((node) => ({
            target: node.target,
            summary: node.failureSummary,
          })),
        })),
        screen,
      ).toEqual([]);
    }
  });
}
