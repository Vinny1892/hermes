import { test, expect } from '@playwright/test';
import { readFileSync } from 'fs';
import * as path from 'path';
import { uploadViaApi, waitForWasm } from '../fixtures/helpers';

const FIXTURE_SMALL = path.resolve(__dirname, '../test-fixtures/small.txt');

test.describe('Server upload flow', () => {
  test('home page renders mode selector after WASM hydration', async ({ page }) => {
    await page.goto('/');
    await waitForWasm(page, '.mode-selector');
    await expect(page.locator('.mode-selector')).toBeVisible();
    // Both mode buttons should be present
    await expect(page.locator('.mode-btn')).toHaveCount(2);
  });

  test('POST /api/upload returns a valid file_id UUID', async ({ request }) => {
    const content = readFileSync(FIXTURE_SMALL);
    const fileId = await uploadViaApi(request, 'small.txt', content);
    expect(fileId).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i,
    );
  });

  test('download page shows file card with name and size', async ({ page, request }) => {
    const content = readFileSync(FIXTURE_SMALL);
    const fileId = await uploadViaApi(request, 'small.txt', content);

    await page.goto(`/d/${fileId}`);
    await waitForWasm(page, '.file-card');

    await expect(page.locator('.file-card')).toBeVisible();
    await expect(page.locator('.file-card-name')).toContainText('small.txt');
    // Size metadata should mention bytes
    await expect(page.locator('.file-meta-value').first()).toBeVisible();
  });

  test('GET /f/:file_id streams correct bytes with Content-Disposition header', async ({
    request,
  }) => {
    const content = readFileSync(FIXTURE_SMALL);
    const originalSize = content.byteLength;
    const fileId = await uploadViaApi(request, 'small.txt', content);

    const response = await request.get(`/f/${fileId}`);
    expect(response.status()).toBe(200);

    const disposition = response.headers()['content-disposition'] ?? '';
    expect(disposition).toMatch(/attachment/i);
    expect(disposition).toMatch(/small\.txt/);

    const body = await response.body();
    expect(body.byteLength).toBe(originalSize);
  });

  test('download page for unknown file_id shows error state', async ({ page }) => {
    await page.goto('/d/00000000-0000-0000-0000-000000000000');
    await waitForWasm(page, '.error');
    await expect(page.locator('.error')).toBeVisible();
    await expect(page.locator('.not-found-link')).toBeVisible();
  });
});
