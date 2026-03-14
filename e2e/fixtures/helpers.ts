import { APIRequestContext, Page } from '@playwright/test';

/**
 * Wait for WASM hydration to complete by polling for a selector that only
 * appears after the Dioxus runtime initialises the virtual DOM.
 */
export async function waitForWasm(
  page: Page,
  selector = '.mode-selector',
  timeout = 30_000,
): Promise<void> {
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
