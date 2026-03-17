import { test, expect, Browser } from '@playwright/test';
import { writeFileSync } from 'fs';
import * as path from 'path';
import * as os from 'os';
import { waitForWasm, waitForP2pStatus, STORAGE_STATE } from '../fixtures/helpers';

// Generate a ~200 KB binary fixture once per run
const FIXTURE_MEDIUM = path.join(os.tmpdir(), 'hermes-e2e-medium.bin');
const MEDIUM_SIZE = 200 * 1024;

test.beforeAll(() => {
  const buf = Buffer.alloc(MEDIUM_SIZE);
  for (let i = 0; i < MEDIUM_SIZE; i++) buf[i] = i & 0xff;
  writeFileSync(FIXTURE_MEDIUM, buf);
});

/**
 * Helper: navigate to home, select P2P mode, wait for the receiver URL.
 * Returns the receiver path (e.g. "/receive/{uuid}").
 * Assumes the page's context already has the auth token (via storageState).
 */
async function setupSenderAndGetReceiverPath(page: import('@playwright/test').Page): Promise<string> {
  await page.goto('/');
  await waitForWasm(page, '.mode-selector');

  // Click the P2P mode button (second .mode-btn label)
  await page.locator('.mode-btn').nth(1).click();

  // The share-link-code inside .p2p-share-container appears once the
  // session is created and the origin eval resolves (should be <10s).
  const shareLinkCode = page.locator('.p2p-share-container .share-link-code');
  await expect(shareLinkCode).toBeVisible({ timeout: 20_000 });

  const receiverUrl = (await shareLinkCode.textContent()) ?? '';
  expect(receiverUrl).toContain('/receive/');

  return receiverUrl.trim().startsWith('http')
    ? new URL(receiverUrl.trim()).pathname
    : receiverUrl.trim();
}

test.describe('P2P WebRTC transfer', () => {
  /**
   * Smoke test: sender opens P2P mode → receiver URL appears →
   * receiver page renders the status card.
   */
  test('receiver page renders status card when session URL is visited', async ({
    browser,
  }: {
    browser: Browser;
  }) => {
    // Give this test extra time: WASM hydration × 2 + WebSocket setup
    test.setTimeout(120_000);

    const senderCtx = await browser.newContext({ storageState: STORAGE_STATE });
    const receiverCtx = await browser.newContext({ storageState: STORAGE_STATE });

    try {
      const senderPage = await senderCtx.newPage();
      const receiverPath = await setupSenderAndGetReceiverPath(senderPage);

      const receiverPage = await receiverCtx.newPage();
      await receiverPage.goto(receiverPath);
      await waitForWasm(receiverPage, '.receive-status-card');

      await expect(receiverPage.locator('.receive-status-card')).toBeVisible();
    } finally {
      await senderCtx.close().catch(() => {});
      await receiverCtx.close().catch(() => {});
    }
  });

  /**
   * Full transfer test: sender and receiver connect via WebRTC DataChannel,
   * sender uploads a ~200 KB file, receiver gets a download link.
   */
  test('full P2P file transfer completes successfully', async ({
    browser,
  }: {
    browser: Browser;
  }) => {
    test.setTimeout(180_000);

    const senderCtx = await browser.newContext({ storageState: STORAGE_STATE });
    const receiverCtx = await browser.newContext({ storageState: STORAGE_STATE });

    try {
      // --- Sender setup ---
      const senderPage = await senderCtx.newPage();
      const receiverPath = await setupSenderAndGetReceiverPath(senderPage);

      // Wait for the sender's signaling WebSocket to open
      // ("Waiting for receiver to connect…" is set right after WS opens)
      await waitForP2pStatus(senderPage, 'Waiting for receiver', 20_000);

      // --- Receiver setup ---
      const receiverPage = await receiverCtx.newPage();
      await receiverPage.goto(receiverPath);
      await waitForWasm(receiverPage, '.receive-status-card');

      // --- Wait for DataChannel to open (connected badge on sender) ---
      // The badge renders when `sender_connected` becomes true, which fires
      // after `startP2pSender` returns (WebSocket + DataChannel open).
      await expect(senderPage.locator('.p2p-connected-badge')).toBeVisible({
        timeout: 30_000,
      });

      // --- Send file ---
      await senderPage.locator('#p2p-file-input').setInputFiles(FIXTURE_MEDIUM);

      // --- Wait for transfer to complete ---
      await waitForP2pStatus(senderPage, 'File sent successfully', 120_000);

      // Receiver should have a download link injected by webrtc.js
      const downloadLink = receiverPage.locator('#p2p-download a');
      await expect(downloadLink).toBeVisible({ timeout: 120_000 });

      // Verify filename
      const downloadAttr = await downloadLink.getAttribute('download');
      expect(downloadAttr).toBeTruthy();

      // Verify the blob URL contains the correct number of bytes
      const blobUrl = await downloadLink.getAttribute('href');
      expect(blobUrl).toMatch(/^blob:/);

      const byteLength = await receiverPage.evaluate(async (url: string) => {
        const res = await fetch(url);
        const buf = await res.arrayBuffer();
        return buf.byteLength;
      }, blobUrl!);

      expect(byteLength).toBe(MEDIUM_SIZE);
    } finally {
      await senderCtx.close().catch(() => {});
      await receiverCtx.close().catch(() => {});
    }
  });
});
