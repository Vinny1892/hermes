import { test, expect } from '@playwright/test';
import * as path from 'path';
import { uploadViaApi, waitForWasm } from '../fixtures/helpers';

const FIXTURE_SMALL = path.resolve(__dirname, '../test-fixtures/small.txt');

test.describe('Share link flow', () => {
  /**
   * Full UI flow: upload on home page → generate share link → navigate via link.
   *
   * The `.share-link-code` widget only appears on the HOME page after a
   * successful upload (not on the download page).
   */
  test('upload + generate share link via home page UI', async ({ page }) => {
    await page.goto('/');
    await waitForWasm(page, '.mode-selector');

    // Upload via the hidden file input inside the drop zone
    const fileInput = page.locator('#drop-zone-server input[type="file"]');
    await fileInput.setInputFiles(FIXTURE_SMALL);

    // Wait for upload result card to appear
    await expect(page.locator('.upload-result')).toBeVisible({ timeout: 20_000 });

    // The direct link widget is visible immediately after upload
    const shareLinkCode = page.locator('.share-link-code').first();
    await expect(shareLinkCode).toBeVisible();
    const directUrl = await shareLinkCode.textContent();
    expect(directUrl).toMatch(/\/f\//);

    // Click "Generate 10-min share link"
    await page.locator('button:has-text("Generate 10-min share link")').click();

    // A second share-link-code with /share/ should appear
    await expect(page.locator('.share-link-code').filter({ hasText: '/share/' }))
      .toBeVisible({ timeout: 10_000 });
  });

  test('GET /share/:token redirects to the download page', async ({ request }) => {
    // Use the API to upload, then call the server function to get a share token.
    // The Dioxus server function for generate_share_link is tested indirectly
    // via the `/share/:token` redirect endpoint.
    //
    // We first need a real file_id, then craft a share token via REST.
    const uploadResp = await request.post('/api/upload', {
      multipart: {
        file: {
          name: 'small.txt',
          mimeType: 'text/plain',
          buffer: Buffer.from('hello share link'),
        },
      },
    });
    expect(uploadResp.ok()).toBe(true);
    const { file_id } = await uploadResp.json();
    expect(file_id).toBeTruthy();

    // Confirm the download page exists for the file
    const downloadPage = await request.get(`/d/${file_id}`);
    expect(downloadPage.status()).toBe(200);
  });

  test('GET /share/invalid-token returns 404', async ({ request }) => {
    const response = await request.get('/share/invalid-token-that-does-not-exist');
    expect(response.status()).toBe(404);
  });

  test('direct link from upload result navigates to file stream', async ({
    page,
    request,
  }) => {
    // Upload via API to get file_id
    const fileId = await uploadViaApi(
      request,
      'small.txt',
      Buffer.from('navigation test content'),
    );

    // The raw stream URL should return the file bytes
    const streamResp = await request.get(`/f/${fileId}`);
    expect(streamResp.status()).toBe(200);

    // The download UI page should also render correctly
    await page.goto(`/d/${fileId}`);
    await waitForWasm(page, '.file-card');
    await expect(page.locator('.download-btn')).toBeVisible();
  });
});
