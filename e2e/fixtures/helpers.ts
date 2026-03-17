import { APIRequestContext, Page } from '@playwright/test';
import * as os from 'os';
import * as path from 'path';

/** Path where the authenticated storage state (localStorage) is saved by auth.setup.ts. */
export const STORAGE_STATE = path.join(os.tmpdir(), 'hermes-e2e-auth.json');

export const E2E_ADMIN_EMAIL = 'admin@hermes.local';
export const E2E_ADMIN_PASSWORD = 'e2e-test-pass';

/**
 * Wait for WASM hydration to complete.
 *
 * The App component sets `data-wasm-ready="true"` on <html> via use_effect,
 * which only runs in the WASM target (not during SSR). We wait for that
 * attribute first, then for the expected selector.
 */
export async function waitForWasm(
  page: Page,
  selector = '.mode-selector',
  timeout = 30_000,
): Promise<void> {
  await page.waitForSelector('html[data-wasm-ready="true"]', { timeout });
  await page.waitForSelector(selector, { state: 'visible', timeout });
}

/**
 * Upload a file directly via the REST API (bypasses the browser UI).
 * Returns the file_id from the server response.
 */
export async function uploadViaApi(
  request: APIRequestContext,
  filename: string,
  content: Buffer | string,
): Promise<string> {
  const response = await request.post('/api/upload', {
    multipart: {
      file: {
        name: filename,
        mimeType: 'application/octet-stream',
        buffer: Buffer.isBuffer(content) ? content : Buffer.from(content),
      },
    },
  });

  if (!response.ok()) {
    throw new Error(
      `Upload failed: ${response.status()} ${await response.text()}`,
    );
  }

  const body = await response.json();
  if (!body.file_id) {
    throw new Error(`Upload response missing file_id: ${JSON.stringify(body)}`);
  }
  return body.file_id as string;
}

/**
 * Log in via the UI form and wait for the redirect to home.
 */
export async function loginViaUi(
  page: Page,
  email = E2E_ADMIN_EMAIL,
  password = E2E_ADMIN_PASSWORD,
): Promise<void> {
  await page.goto('/login');
  // Wait for the WASM sentinel — ensures event handlers are attached before
  // we interact with the form (SSR renders the form but not the sentinel).
  await page.waitForSelector('html[data-wasm-ready="true"]', { timeout: 30_000 });
  await page.locator('#l-email').fill(email);
  await page.locator('#l-pass').fill(password);
  await page.locator('.login-btn').click();
  await page.waitForURL(/\/$/, { timeout: 20_000 });
}

/**
 * Click the Logout button in the navbar and wait for the /login redirect.
 */
export async function logout(page: Page): Promise<void> {
  await page.locator('[title="Logout"]').click();
  await page.waitForURL('**/login', { timeout: 10_000 });
}

/**
 * Poll `#p2p-status` until its text contains `expected` or the timeout elapses.
 * webrtc.js writes status updates to this element at runtime.
 */
export async function waitForP2pStatus(
  page: Page,
  expected: string,
  timeout = 30_000,
): Promise<void> {
  await page.waitForFunction(
    ({ selector, text }: { selector: string; text: string }) => {
      const el = document.querySelector(selector);
      return el?.textContent?.includes(text) ?? false;
    },
    { selector: '#p2p-status', text: expected },
    { timeout },
  );
}
